//! The composition root.
//!
//! The one place that constructs the engine bridge, the Model, the renderer
//! and the window, injecting each downward. No other crate builds a layer
//! other than its own.
//!
//! Milestone 2: a window showing a document, with a camera that responds. The
//! interface chrome and the sculpting loop follow.

#![forbid(unsafe_code)]

use std::sync::Arc;

use clayspace_model::claycore::{self, Document, Mesh};
use clayspace_view::{
    Camera, Gpu, GpuMesh, Overlays, Renderer, SurfaceLoss, Vertex, ViewPreset, WindowSurface,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

fn main() {
    let document = starting_document();
    report_environment();

    let event_loop = EventLoop::new().expect("create the event loop");
    // Wait for input rather than spinning: nothing animates yet, so redrawing
    // continuously would burn a core for no reason.
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(document);
    event_loop.run_app(&mut app).expect("run the application");
}

/// What the app opens with until documents can be opened.
fn starting_document() -> Document {
    let mut doc = Document::new().expect("create a document");
    let layer = doc.add_sdf_layer("Forma").expect("add a layer");
    let body = claycore::Item::sphere(1.0).expect("sphere");
    doc.add_item(layer, &body).expect("place the body");

    let mut head = claycore::Item::sphere(0.55).expect("sphere");
    head.set_position([0.0, 1.05, 0.0]).expect("position");
    head.set_blend(claycore::Blend::Quadratic, 0.35).expect("blend");
    doc.add_item(layer, &head).expect("place the head");

    doc.enable_undo().expect("enable undo");
    doc
}

/// Prints what the engine and the renderer each resolved to.
///
/// They are chosen independently, so a GPU for one says nothing about the
/// other, and a user reporting a performance problem needs both.
fn report_environment() {
    println!("ClaySpaceDesktop {}", env!("CARGO_PKG_VERSION"));
    println!("  engine   : claycore {}", claycore::version());
    match claycore::backends() {
        Ok(found) => {
            let names: Vec<_> = found.iter().map(ToString::to_string).collect();
            println!("  backends : {}", names.join(", "));
        }
        Err(e) => println!("  backends : discovery failed: {e}"),
    }
}

/// What the pointer is currently doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drag {
    None,
    Orbit,
    Pan,
}

struct App {
    document: Document,
    camera: Camera,
    /// Kept apart from the graphics so that losing the device does not lose
    /// the window with it.
    window: Option<Arc<Window>>,
    /// Built once the window exists; a device cannot precede a surface.
    graphics: Option<Graphics>,
    drag: Drag,
    last_cursor: Option<(f64, f64)>,
}

struct Graphics {
    gpu: Gpu,
    surface: WindowSurface,
    renderer: Renderer,
    mesh: GpuMesh,
}

impl App {
    fn new(document: Document) -> Self {
        Self {
            document,
            camera: Camera::default(),
            window: None,
            graphics: None,
            drag: Drag::None,
            last_cursor: None,
        }
    }

    /// Meshes the document and uploads it, framing the camera on the result.
    fn rebuild_geometry(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        // The export mesher for now; the interactive path meshes the brick
        // cache's dirty subset instead, and arrives with the sculpting loop.
        let mesh = match self.document.mesh(claycore::MeshParams {
            resolution: 96,
            ..Default::default()
        }) {
            Ok(mesh) => mesh,
            Err(e) => {
                // An empty document is refused rather than meshed. That is
                // nothing to draw, not a failure.
                eprintln!("nothing to draw: {e}");
                graphics.mesh = GpuMesh::new(&graphics.gpu);
                return;
            }
        };

        let (vertices, indices) = to_vertices(&mesh);
        graphics.mesh.upload(&graphics.gpu, &vertices, &indices);
        if let Some((min, max)) = graphics.mesh.bounds() {
            self.camera.frame_bounds(min, max);
        } else {
            self.camera.frame_default();
        }
    }

    fn redraw(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let frame = match graphics.surface.acquire(&graphics.gpu) {
            Ok(frame) => frame,
            Err(SurfaceLoss::Skip | SurfaceLoss::Reconfigure) => return,
            Err(SurfaceLoss::DeviceLost) => {
                // The document lives in host memory, so nothing authored is
                // lost; only the GPU-side resources need rebuilding.
                eprintln!("the graphics device was lost; rebuilding rendering");
                self.graphics = None;
                if self.create_graphics() {
                    self.request_redraw();
                } else {
                    eprintln!("rendering could not be restarted");
                }
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        graphics.renderer.render(
            &graphics.gpu,
            &view,
            graphics.surface.framebuffer(),
            &self.camera,
            &graphics.mesh,
            false,
        );
        frame.present();
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Builds every GPU-side resource for the current window.
    ///
    /// Called at startup and again after a device loss. The document is not
    /// touched, because none of it lives on the GPU — losing the device costs
    /// the frame in flight and nothing the user authored.
    fn create_graphics(&mut self) -> bool {
        let Some(window) = self.window.clone() else {
            return false;
        };
        let (gpu, surface) = match pollster::block_on(WindowSurface::new(window)) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("could not start rendering: {e}");
                return false;
            }
        };

        let mut renderer = Renderer::new(&gpu, surface.format());
        renderer.set_overlays(&gpu, Overlays::default(), 4.0);
        let mesh = GpuMesh::new(&gpu);

        self.graphics = Some(Graphics {
            gpu,
            surface,
            renderer,
            mesh,
        });
        self.rebuild_geometry();
        true
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("Sculptor 3D")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        self.window = Some(Arc::new(
            event_loop
                .create_window(attributes)
                .expect("create the window"),
        ));

        if !self.create_graphics() {
            event_loop.exit();
            return;
        }
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                if let Some(graphics) = self.graphics.as_mut() {
                    let gpu = graphics.gpu.clone();
                    graphics.surface.resize(&gpu, size.width, size.height);
                }
                self.request_redraw();
            }

            WindowEvent::RedrawRequested => self.redraw(),

            WindowEvent::MouseInput { state, button, .. } => {
                self.drag = match (state, button) {
                    (ElementState::Pressed, MouseButton::Left) => Drag::Orbit,
                    (ElementState::Pressed, MouseButton::Middle) => Drag::Pan,
                    (ElementState::Released, _) => Drag::None,
                    _ => self.drag,
                };
                if self.drag == Drag::None {
                    self.last_cursor = None;
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                let current = (position.x, position.y);
                if let (Some(previous), true) = (self.last_cursor, self.drag != Drag::None) {
                    let dx = (current.0 - previous.0) as f32;
                    let dy = (current.1 - previous.1) as f32;
                    match self.drag {
                        Drag::Orbit => self.camera.orbit(dx * 0.008, dy * 0.008),
                        Drag::Pan => self.camera.pan(dx, dy),
                        Drag::None => {}
                    }
                    self.request_redraw();
                }
                self.last_cursor = Some(current);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.02,
                };
                self.camera.zoom(amount);
                self.request_redraw();
            }

            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                match code {
                    KeyCode::Digit1 => self.camera.apply_preset(ViewPreset::Perspective),
                    KeyCode::Digit2 => self.camera.apply_preset(ViewPreset::Front),
                    KeyCode::Digit3 => self.camera.apply_preset(ViewPreset::Side),
                    KeyCode::Digit4 => self.camera.apply_preset(ViewPreset::Top),
                    KeyCode::KeyF => {
                        if let Some((min, max)) =
                            self.graphics.as_ref().and_then(|g| g.mesh.bounds())
                        {
                            self.camera.frame_bounds(min, max);
                        }
                    }
                    KeyCode::KeyM => {
                        if let Some(graphics) = self.graphics.as_mut() {
                            let next = next_matcap(graphics.renderer.matcap());
                            let gpu = graphics.gpu.clone();
                            graphics.renderer.set_matcap(&gpu, next);
                        }
                    }
                    KeyCode::Escape => event_loop.exit(),
                    _ => return,
                }
                self.request_redraw();
            }

            _ => {}
        }
    }
}

fn next_matcap(current: clayspace_view::MatCap) -> clayspace_view::MatCap {
    let all = clayspace_view::MatCap::ALL;
    let index = all.iter().position(|m| *m == current).unwrap_or(0);
    all[(index + 1) % all.len()]
}

/// Moves an engine mesh into the renderer's vertex layout in one pass.
fn to_vertices(mesh: &Mesh) -> (Vec<Vertex>, Vec<u32>) {
    let count = mesh.vertex_count();
    let mut bytes = vec![0u8; count * Vertex::STRIDE];

    let has_colors = mesh.colors().is_some();
    if !has_colors {
        // The engine refuses a layout naming an attribute the mesh does not
        // carry, so white is written here and the copy writes around it.
        for vertex in bytes.chunks_exact_mut(Vertex::STRIDE) {
            for channel in 0..3 {
                let at = Vertex::COLOR_OFFSET + channel * 4;
                vertex[at..at + 4].copy_from_slice(&1.0f32.to_le_bytes());
            }
        }
    }

    let layout = claycore::VertexLayout {
        stride: Some(Vertex::STRIDE as u32),
        position_offset: Some(Vertex::POSITION_OFFSET as i32),
        normal_offset: Some(Vertex::NORMAL_OFFSET as i32),
        color_offset: has_colors.then_some(Vertex::COLOR_OFFSET as i32),
        uv_offset: None,
    };
    mesh.copy_vertices(layout, &mut bytes)
        .expect("copy vertices into the renderer's layout");

    let read = |v: &[u8], offset: usize| -> [f32; 3] {
        std::array::from_fn(|i| {
            let at = offset + i * 4;
            f32::from_le_bytes(v[at..at + 4].try_into().unwrap())
        })
    };
    let vertices = bytes
        .chunks_exact(Vertex::STRIDE)
        .map(|v| Vertex {
            position: read(v, Vertex::POSITION_OFFSET),
            normal: read(v, Vertex::NORMAL_OFFSET),
            color: read(v, Vertex::COLOR_OFFSET),
        })
        .collect();

    let mut indices = vec![0u32; mesh.index_count()];
    mesh.copy_indices(&mut indices).expect("copy indices");
    (vertices, indices)
}
