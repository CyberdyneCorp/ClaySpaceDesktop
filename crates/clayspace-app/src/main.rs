//! The composition root.
//!
//! The one place that constructs the engine bridge, the Model, the ViewModels,
//! the renderer and the window, and injects each downward. No other crate
//! builds a layer other than its own.

#![forbid(unsafe_code)]

use std::sync::Arc;

use clayspace_app::{ray_at, SharedDocument, SurfaceGeometry, ViewportInput};
use clayspace_engine::{claycore, BackendPolicy, ClayDocument};
use clayspace_model::ViewPresetKind;
use clayspace_view::shell::{self, region, ShellState};
use clayspace_view::{
    mirrored_cursors, BrushCursor, Camera, Gpu, Locale, MatCap, Overlays, Renderer, Strings,
    SurfaceLoss, ViewPreset, WindowSurface,
};
use clayspace_vm::{
    Axis, Command, CommandQueue, DocumentViewModel, Guard, SceneViewModel, SculptViewModel,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

fn main() {
    let policy = match BackendPolicy::discover(None) {
        Ok(policy) => policy,
        Err(e) => {
            eprintln!("the engine's backends could not be discovered: {e}");
            return;
        }
    };
    report(&policy);

    let document = match ClayDocument::new(policy.clone()).and_then(ClayDocument::with_starting_form)
    {
        Ok(document) => document,
        Err(e) => {
            eprintln!("the starting document could not be built: {e}");
            return;
        }
    };

    let event_loop = EventLoop::new().expect("create the event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(SharedDocument::new(document), policy);
    event_loop.run_app(&mut app).expect("run the application");
}

fn report(policy: &BackendPolicy) {
    println!("ClaySpaceDesktop {}", env!("CARGO_PKG_VERSION"));
    println!("  engine   : claycore {}", claycore::version());
    let names: Vec<_> = policy.available().iter().map(ToString::to_string).collect();
    println!("  backends : {}", names.join(", "));
    println!("  active   : {} ({:?})", policy.active(), policy.reason());
}

/// What the pointer is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drag {
    None,
    Sculpt,
    Orbit,
    Pan,
}

struct App {
    document: SharedDocument,
    sculpt: SculptViewModel,
    scene: SceneViewModel,
    document_vm: DocumentViewModel,
    policy: BackendPolicy,

    camera: Camera,
    window: Option<Arc<Window>>,
    graphics: Option<Graphics>,

    drag: Drag,
    /// Where the pointer last met the surface, and the direction it arrived
    /// from.
    ///
    /// Kept rather than the finished cursor so the ring can be rebuilt from
    /// the *current* brush size and symmetry every frame. Storing the ring
    /// instead left it stale until the pointer moved again, which is a cursor
    /// that lies about what the next click will do.
    hover: Option<([f32; 3], [f32; 3])>,
    /// The viewport's rectangle in egui points, which is what a pointer
    /// position from egui arrives in.
    viewport: Option<egui::Rect>,
    /// The symmetry the overlays were last built for.
    overlay_symmetry: [bool; 3],
    strings: &'static Strings,
}

struct Graphics {
    gpu: Gpu,
    surface: WindowSurface,
    renderer: Renderer,
    geometry: SurfaceGeometry,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl App {
    fn new(document: SharedDocument, policy: BackendPolicy) -> Self {
        let sculpt = SculptViewModel::new(Box::new(document.clone()));
        let scene = SceneViewModel::new(Box::new(document.clone()));
        let document_vm = DocumentViewModel::new(Box::new(document.clone()), "Sem título");
        Self {
            document,
            sculpt,
            scene,
            document_vm,
            policy,
            camera: Camera::default(),
            window: None,
            graphics: None,
            drag: Drag::None,
            hover: None,
            viewport: None,
            overlay_symmetry: [false; 3],
            strings: Strings::for_locale(Locale::default()),
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Builds every GPU-side resource. Called at startup and after device loss.
    fn create_graphics(&mut self) -> bool {
        let Some(window) = self.window.clone() else {
            return false;
        };
        let (gpu, surface) = match pollster::block_on(WindowSurface::new(window.clone())) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("could not start rendering: {e}");
                return false;
            }
        };

        let mut renderer = Renderer::new(&gpu, surface.format());
        renderer.set_overlays(&gpu, Overlays::default(), 3.0);
        renderer.show_gizmo = true;

        let context = egui::Context::default();
        shell::apply_theme(&context);
        let egui_state = egui_winit::State::new(
            context,
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(&gpu.device, surface.format(), None, 1, false);

        let geometry = SurfaceGeometry::new(&gpu);
        self.graphics = Some(Graphics {
            gpu,
            surface,
            renderer,
            geometry,
            egui_state,
            egui_renderer,
        });

        self.sync_geometry();
        self.frame_all();
        true
    }

    /// Re-meshes whatever the document reports as dirty.
    fn sync_geometry(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let gpu = graphics.gpu.clone();
        let result = self
            .document
            .with(|document| graphics.geometry.sync(&gpu, document));
        match result {
            Ok(_) => self.sculpt.acknowledge_remesh(),
            Err(e) => eprintln!("the surface could not be re-meshed: {e}"),
        }
    }

    /// Saves, asking for a path when there is not one yet.
    fn save(&mut self, ask_for_path: bool) {
        let known = self.document_vm.path().get().clone();
        let path = match (known, ask_for_path) {
            (Some(path), false) => Some(path),
            _ => rfd::FileDialog::new()
                .set_title("Salvar escultura")
                .add_filter("ClaySpace", &["clayspace"])
                .set_file_name(format!("{}.clayspace", self.document_vm.name().get()))
                .save_file(),
        };
        let Some(path) = path else {
            return; // Cancelled. Not a failure, and nothing to report.
        };
        if let Err(e) = self.document_vm.save_as(&path) {
            eprintln!("{e}");
        }
        self.request_redraw();
    }

    /// Opens a document, after asking about unsaved work.
    fn open(&mut self) {
        if self.document_vm.guard() == Guard::WouldLoseWork && !self.confirm_discarding_work() {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title("Abrir escultura")
            .add_filter("ClaySpace", &["clayspace"])
            .pick_file()
        else {
            return;
        };
        match self.document_vm.open(&path) {
            Ok(()) => self.after_document_replaced(),
            Err(e) => eprintln!("{e}"),
        }
        self.request_redraw();
    }

    /// Starts a new document, after asking about unsaved work.
    fn new_document(&mut self) {
        if self.document_vm.guard() == Guard::WouldLoseWork && !self.confirm_discarding_work() {
            return;
        }
        match self.document_vm.new_document() {
            Ok(()) => self.after_document_replaced(),
            Err(e) => eprintln!("{e}"),
        }
        self.request_redraw();
    }

    /// Asks whether unsaved work may be thrown away.
    ///
    /// A native dialog rather than something drawn in the shell: this is the
    /// one question in the application whose answer cannot be undone, and the
    /// platform's own dialog is the one a user already knows how to read.
    fn confirm_discarding_work(&self) -> bool {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Alterações não salvas")
            .set_description("A escultura tem alterações que não foram salvas. Descartar?")
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes
    }

    /// Everything that has to catch up when the document underneath changes.
    fn after_document_replaced(&mut self) {
        self.scene.refresh();
        self.sculpt.forget_history();
        if let Some(graphics) = self.graphics.as_mut() {
            let gpu = graphics.gpu.clone();
            // A rebuild rather than a sync: nothing about the old document's
            // dirty set describes the new one.
            if let Err(e) = self
                .document
                .with(|document| graphics.geometry.rebuild(&gpu, document))
            {
                eprintln!("the surface could not be meshed: {e}");
            }
        }
        self.frame_all();
    }

    /// Re-meshes the whole surface after a gesture, clearing the seams the
    /// per-segment path leaves.
    fn settle_geometry(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let gpu = graphics.gpu.clone();
        let result = self
            .document
            .with(|document| graphics.geometry.settle(&gpu, document));
        match result {
            Ok(()) => self.sculpt.acknowledge_remesh(),
            Err(e) => eprintln!("the surface could not be re-meshed: {e}"),
        }
    }

    fn frame_all(&mut self) {
        match self.sculpt.bounds() {
            Some((min, max)) => self.camera.frame_bounds(min.into(), max.into()),
            None => self.camera.frame_default(),
        }
    }

    /// Turns a pointer position into a ray, when it is over the viewport.
    fn ray_at(&self, point: egui::Pos2) -> Option<([f32; 3], [f32; 3])> {
        ray_at(&self.camera, self.viewport?, point)
    }

    /// Where the surface is under a pointer position, if it is there at all.
    fn pick_at(&self, point: egui::Pos2) -> Option<([f32; 3], [f32; 3])> {
        let (origin, direction) = self.ray_at(point)?;
        let hit = self.sculpt.pick(origin, direction)?;
        // Toward the camera: the pick reports where the surface is, not which
        // way it faces, and a ring in the view plane reads correctly from any
        // angle.
        Some((hit, [-direction[0], -direction[1], -direction[2]]))
    }

    /// The rings to draw: the pointer's, plus one for every place symmetry
    /// will also deposit the dab.
    fn cursors(&self) -> Vec<BrushCursor> {
        let Some((position, normal)) = self.hover else {
            return Vec::new();
        };
        let cursor = BrushCursor {
            position,
            normal,
            radius: self.sculpt.brush().get().size,
            mirrored: false,
        };
        mirrored_cursors(cursor, *self.sculpt.symmetry().get())
    }

    /// Rebuilds the mirror-plane overlays when the symmetry changes.
    ///
    /// The planes cost a rebuild, so this is not done every frame — but it is
    /// checked every frame, because symmetry can be toggled from the options
    /// bar as well as the keyboard and neither should need a nudge to show.
    fn sync_symmetry_overlay(&mut self) {
        let symmetry = *self.sculpt.symmetry().get();
        if symmetry == self.overlay_symmetry {
            return;
        }
        self.overlay_symmetry = symmetry;
        if let Some(graphics) = self.graphics.as_mut() {
            let gpu = graphics.gpu.clone();
            graphics.renderer.set_overlays(
                &gpu,
                Overlays {
                    grid: true,
                    symmetry_planes: symmetry,
                },
                3.0,
            );
        }
    }

    /// Sends a stroke sample at the pointer, if it met the surface.
    fn stroke_at(&mut self, point: egui::Pos2, begin: bool) {
        let Some((position, _)) = self.pick_at(point) else {
            return;
        };
        let command = if begin {
            Command::BeginStroke {
                position,
                pressure: 1.0,
            }
        } else {
            Command::ContinueStroke {
                position,
                pressure: 1.0,
            }
        };
        self.apply(command);
    }

    /// Acts on one frame's worth of viewport input.
    ///
    /// Routing lives here rather than in the winit handler because the panels
    /// decide where the viewport is, and only egui knows whether a press
    /// landed on a widget. Asking winit produced a version that dropped every
    /// press in the window.
    fn drive(&mut self, input: &ViewportInput) {
        // Hover first, and unconditionally: the ring should follow the pointer
        // during a stroke as well as before one.
        if let Some(point) = input.pointer {
            self.hover = self.pick_at(point);
        }

        if input.released && self.drag != Drag::None {
            if self.drag == Drag::Sculpt {
                self.apply(Command::EndStroke);
            }
            self.drag = Drag::None;
        }

        if let (Some(point), Some(button), true) = (input.pointer, input.pressed, input.over_viewport)
        {
            let on_surface = self.pick_at(point).is_some();
            let started = match button {
                egui::PointerButton::Middle => Drag::Pan,
                egui::PointerButton::Secondary => Drag::Orbit,
                // On the surface it sculpts; off it, it orbits. That is
                // ZBrush's rule, and it is the only one that leaves a Mac
                // trackpad — which has no comfortable right-drag — able to
                // turn the model.
                _ if input.orbit_modifier || !on_surface => Drag::Orbit,
                _ => Drag::Sculpt,
            };
            self.drag = started;
            if started == Drag::Sculpt {
                self.stroke_at(point, true);
            }
        }

        if self.drag != Drag::None && input.delta != egui::Vec2::ZERO {
            match self.drag {
                Drag::Sculpt => {
                    if let Some(point) = input.pointer {
                        self.stroke_at(point, false);
                    }
                }
                Drag::Orbit => self
                    .camera
                    .orbit(input.delta.x * 0.008, input.delta.y * 0.008),
                Drag::Pan => self.camera.pan(input.delta.x, input.delta.y),
                Drag::None => {}
            }
        }

        if input.over_viewport && input.scroll != 0.0 {
            self.camera.zoom(input.scroll);
        }
    }

    /// Dispatches a command to whichever ViewModel owns it.
    fn apply(&mut self, command: Command) {
        if let Err(e) = self.sculpt.dispatch(command.clone()) {
            // A refusal is not swallowed; the tool status carries the reason
            // to the options bar, and this records it for the log.
            eprintln!("{e}");
        }
        if let Err(e) = self.scene.dispatch(&command) {
            eprintln!("{e}");
        }
        if command.touches_document() {
            self.scene.refresh();
            self.sync_geometry();
            // The title bar's "não salvo" is driven from here rather than
            // inferred inside the document ViewModel, which never sees a
            // sculpting command and would have to guess.
            self.document_vm.touched();
        }
        // The end of a gesture is where the fast path's approximation is paid
        // off: a full re-mesh, once, rather than the slivers it leaves along
        // the seams of the edit. Affordable here because the pointer is up and
        // nobody is waiting on the next sample.
        if matches!(command, Command::EndStroke | Command::CancelStroke) {
            self.settle_geometry();
        }
        self.request_redraw();
    }

    fn redraw(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if self.graphics.is_none() {
            return;
        }

        // The interface is built first, because it decides where the viewport
        // is and therefore what a pointer position means.
        let raw_input = self
            .graphics
            .as_mut()
            .expect("graphics")
            .egui_state
            .take_egui_input(&window);
        let context = self
            .graphics
            .as_ref()
            .expect("graphics")
            .egui_state
            .egui_ctx()
            .clone();

        let scene = self.scene.scene().get().clone();
        let materials: Vec<&str> = MatCap::ALL.iter().map(|m| m.label()).collect();
        let material = self
            .graphics
            .as_ref()
            .map(|g| g.renderer.matcap().label())
            .unwrap_or("");
        let backend = self.policy.active().to_string();
        let memory = self
            .document
            .with(|document| document.cache().stats().ok())
            .map(|stats| (stats.memory_usage, stats.memory_budget.unwrap_or(0)))
            .unwrap_or((0, 0));
        let document_name = self.document_vm.name().get().clone();
        let last = self.sculpt.last_action().get().clone();
        let history = *self.sculpt.history().get();

        let mut queue = CommandQueue::new();
        let mut viewport = None;
        let mut input = ViewportInput::default();
        let state = ShellState {
            strings: self.strings,
            document_name: document_name.as_str(),
            modified: *self.document_vm.modified().get(),
            tool: *self.sculpt.tool().get(),
            brush: *self.sculpt.brush().get(),
            tool_status: self.sculpt.tool_status().get().as_deref(),
            symmetry: *self.sculpt.symmetry().get(),
            scene: &scene,
            stats: *self.sculpt.stats().get(),
            view_preset: *self.sculpt.view_preset().get(),
            material,
            materials: &materials,
            can_undo: history.can_undo,
            can_redo: history.can_redo,
            memory,
            backend: &backend,
            units: "mm",
            last_action: (!last.label.is_empty()).then_some((last.label.as_str(), last.changed)),
        };

        let output = context.run(raw_input, |ctx| {
            egui::TopBottomPanel::top("menu")
                .exact_height(region::MENU_BAR)
                .show(ctx, |ui| shell::menu_bar(ui, &state, &mut queue));
            egui::TopBottomPanel::top("options")
                .exact_height(region::OPTIONS_BAR)
                .show(ctx, |ui| shell::options_bar(ui, &state, &mut queue));
            egui::TopBottomPanel::bottom("status")
                .exact_height(region::STATUS)
                .show(ctx, |ui| shell::status_bar(ui, &state));
            egui::TopBottomPanel::bottom("shelf")
                .exact_height(region::SHELF)
                .show(ctx, |ui| {
                    egui::ScrollArea::horizontal()
                        .show(ui, |ui| shell::brush_shelf(ui, &state, &mut queue));
                });
            egui::SidePanel::left("left")
                .exact_width(region::LEFT)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| shell::left_panel(ui, &state, &mut queue));
                });
            egui::SidePanel::right("right")
                .exact_width(region::RIGHT)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| shell::right_panel(ui, &state, &mut queue));
                });
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        shell::viewport_bar(ui, &state, &mut queue);
                        // Whatever the bar left is the viewport, allocated as
                        // a real region so egui does the hit test. Its rect is
                        // also what the scene is drawn into, so the ray and
                        // the pixels cannot drift apart.
                        let (rect, response) = ui.allocate_exact_size(
                            ui.available_size(),
                            egui::Sense::click_and_drag(),
                        );
                        viewport = Some(rect);
                        input = ViewportInput::read(ui, &response);
                    });
                });
        });

        let scale = context.pixels_per_point();
        self.viewport = viewport;

        drop(state);
        self.drive(&input);
        for command in queue.drain() {
            self.handle(command);
        }

        // The rings are rebuilt every frame from the live brush and symmetry,
        // so `[` and `]` and the mirror toggles show up without the pointer
        // having to move to prompt them.
        let cursors = self.cursors();
        let scene_viewport = self.viewport.map(|rect| {
            [
                rect.min.x * scale,
                rect.min.y * scale,
                rect.width() * scale,
                rect.height() * scale,
            ]
        });
        self.sync_symmetry_overlay();

        let graphics = self.graphics.as_mut().expect("graphics");
        let frame = match graphics.surface.acquire(&graphics.gpu) {
            Ok(frame) => frame,
            Err(SurfaceLoss::Skip | SurfaceLoss::Reconfigure) => return,
            Err(SurfaceLoss::DeviceLost) => {
                eprintln!("the graphics device was lost; rebuilding rendering");
                self.graphics = None;
                if self.create_graphics() {
                    self.request_redraw();
                }
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        graphics.renderer.set_cursors(&graphics.gpu, &cursors);
        graphics.renderer.set_scene_viewport(scene_viewport);
        graphics.renderer.render(
            &graphics.gpu,
            &view,
            graphics.surface.framebuffer(),
            &self.camera,
            graphics.geometry.mesh(),
            false,
        );
        paint_interface(graphics, &context, output, &view, scale);
        frame.present();
    }

    /// Applies a command, plus the view-side effects no ViewModel owns.
    fn handle(&mut self, command: Command) {
        match command {
            Command::FrameAll => {
                self.frame_all();
                self.request_redraw();
            }
            Command::NextMaterial => {
                if let Some(graphics) = self.graphics.as_mut() {
                    let gpu = graphics.gpu.clone();
                    let next = next_matcap(graphics.renderer.matcap());
                    graphics.renderer.set_matcap(&gpu, next);
                }
                self.request_redraw();
            }
            Command::SetViewPreset(preset) => {
                self.camera.apply_preset(match preset {
                    ViewPresetKind::Perspective => ViewPreset::Perspective,
                    ViewPresetKind::Front => ViewPreset::Front,
                    ViewPresetKind::Side => ViewPreset::Side,
                    ViewPresetKind::Top => ViewPreset::Top,
                });
                self.apply(command);
            }
            other => self.apply(other),
        }
    }
}

/// Paints one egui frame over the viewport.
fn paint_interface(
    graphics: &mut Graphics,
    context: &egui::Context,
    output: egui::FullOutput,
    target: &wgpu::TextureView,
    pixels_per_point: f32,
) {
    let primitives = context.tessellate(output.shapes, pixels_per_point);
    for (id, delta) in &output.textures_delta.set {
        graphics
            .egui_renderer
            .update_texture(&graphics.gpu.device, &graphics.gpu.queue, *id, delta);
    }

    let descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [
            graphics.surface.framebuffer().width,
            graphics.surface.framebuffer().height,
        ],
        pixels_per_point,
    };
    let mut encoder = graphics
        .gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("interface"),
        });
    graphics.egui_renderer.update_buffers(
        &graphics.gpu.device,
        &graphics.gpu.queue,
        &mut encoder,
        &primitives,
        &descriptor,
    );
    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("interface"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load, not clear: the sculpt is already there.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        graphics
            .egui_renderer
            .render(&mut pass.forget_lifetime(), &primitives, &descriptor);
    }
    graphics.gpu.queue.submit(Some(encoder.finish()));

    for id in &output.textures_delta.free {
        graphics.egui_renderer.free_texture(id);
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
        // The interface sees input first and says whether it wanted it.
        let consumed = {
            let Some(window) = self.window.clone() else {
                return;
            };
            match self.graphics.as_mut() {
                Some(graphics) => {
                    let response = graphics.egui_state.on_window_event(&window, &event);
                    if response.repaint {
                        window.request_redraw();
                    }
                    response.consumed
                }
                None => false,
            }
        };

        match event {
            WindowEvent::CloseRequested => {
                // The last chance to keep the work. Closing over unsaved edits
                // without asking is the one mistake this application can make
                // that a user cannot undo.
                if self.document_vm.guard() == Guard::Clear || self.confirm_discarding_work() {
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(graphics) = self.graphics.as_mut() {
                    let gpu = graphics.gpu.clone();
                    graphics.surface.resize(&gpu, size.width, size.height);
                }
                self.request_redraw();
            }

            WindowEvent::RedrawRequested => self.redraw(),

            // Pointer input is not handled here. egui owns the hit test —
            // it knows where the panels are — so the viewport reads its own
            // input inside the frame; see `ViewportInput`. All this handler
            // does is make sure a frame happens.
            WindowEvent::MouseInput { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseWheel { .. } => self.request_redraw(),

            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() && !consumed => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };

                // The file shortcuts, which take the platform's command
                // modifier and so are handled before the bare-letter ones.
                let modifiers = self
                    .graphics
                    .as_ref()
                    .map(|g| g.egui_state.egui_ctx().input(|i| i.modifiers))
                    .unwrap_or_default();
                if modifiers.command {
                    match code {
                        KeyCode::KeyS if modifiers.shift => self.save(true),
                        KeyCode::KeyS => self.save(false),
                        KeyCode::KeyO => self.open(),
                        KeyCode::KeyN => self.new_document(),
                        _ => {}
                    }
                    return;
                }

                let command = match code {
                    KeyCode::Digit1 => Some(Command::SetViewPreset(ViewPresetKind::Perspective)),
                    KeyCode::Digit2 => Some(Command::SetViewPreset(ViewPresetKind::Front)),
                    KeyCode::Digit3 => Some(Command::SetViewPreset(ViewPresetKind::Side)),
                    KeyCode::Digit4 => Some(Command::SetViewPreset(ViewPresetKind::Top)),
                    KeyCode::KeyF => Some(Command::FrameAll),
                    KeyCode::KeyM => Some(Command::NextMaterial),
                    KeyCode::KeyX => Some(Command::ToggleSymmetry(Axis::X)),
                    KeyCode::KeyY => Some(Command::ToggleSymmetry(Axis::Y)),
                    KeyCode::KeyZ => Some(Command::Undo),
                    KeyCode::KeyR => Some(Command::Redo),
                    KeyCode::BracketLeft => {
                        Some(Command::SetBrushSize(self.sculpt.brush().get().size * 0.8))
                    }
                    KeyCode::BracketRight => {
                        Some(Command::SetBrushSize(self.sculpt.brush().get().size * 1.25))
                    }
                    KeyCode::Escape => {
                        event_loop.exit();
                        None
                    }
                    _ => None,
                };
                if let Some(command) = command {
                    self.handle(command);
                }
            }

            _ => {}
        }
    }
}

fn next_matcap(current: MatCap) -> MatCap {
    let all = MatCap::ALL;
    let index = all.iter().position(|m| *m == current).unwrap_or(0);
    all[(index + 1) % all.len()]
}
