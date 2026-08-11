//! Proves the window can present a frame, not merely that it can be created.
//!
//! This exists because the offscreen visual tests structurally cannot cover
//! it. They verify the *renderer*; they never build a surface, so they could
//! not catch the bug that made the first real window abort on its first frame.
//!
//! That bug was two `wgpu::Instance`s: the window constructor made one for the
//! surface, and the device constructor quietly made another for the adapter. A
//! surface lives in the registry of the instance that created it, so the
//! device could not present it. Every call reported success and the first
//! presented frame aborted with `Surface does not exist`.
//!
//! This test has been checked against that regression: reintroducing the
//! second instance makes it fail with exactly that panic.
//!
//! The test needs a display and a window server, so it skips where there is
//! none rather than failing. Set `CLAYSPACE_REQUIRE_WINDOW=1` to turn that
//! skip into a failure — which is what a machine that is supposed to have a
//! display should do.

use std::sync::Arc;
use std::time::{Duration, Instant};

use clayspace_view::{Camera, GpuMesh, Overlays, Renderer, SurfaceLoss, WindowSurface};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// How many frames must be presented before the test is satisfied.
///
/// More than one, because the failure this guards against happened on the
/// *first* frame after construction — a test that only built the surface would
/// have passed against the broken version.
const FRAMES_REQUIRED: u32 = 3;

#[derive(Default)]
struct Outcome {
    presented: u32,
    failure: Option<String>,
}

struct SmokeApp {
    window: Option<Arc<Window>>,
    graphics: Option<(clayspace_view::Gpu, WindowSurface, Renderer, GpuMesh)>,
    camera: Camera,
    outcome: Outcome,
    deadline: Instant,
}

impl SmokeApp {
    fn new(deadline: Instant) -> Self {
        Self {
            window: None,
            graphics: None,
            camera: Camera::default(),
            outcome: Outcome::default(),
            deadline,
        }
    }

    fn fail(&mut self, why: impl Into<String>, event_loop: &ActiveEventLoop) {
        if self.outcome.failure.is_none() {
            self.outcome.failure = Some(why.into());
        }
        event_loop.exit();
    }
}

impl ApplicationHandler for SmokeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("ClaySpace smoke test")
            .with_visible(false)
            .with_inner_size(winit::dpi::LogicalSize::new(320.0, 240.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                return self.fail(format!("the window could not be created: {e}"), event_loop)
            }
        };

        let (gpu, surface) = match pollster::block_on(WindowSurface::new(window.clone())) {
            Ok(pair) => pair,
            Err(e) => return self.fail(format!("rendering could not start: {e}"), event_loop),
        };

        let mut renderer = Renderer::new(&gpu, surface.format());
        renderer.set_overlays(&gpu, Overlays::default(), 4.0);

        // Real geometry, so the frame exercises the pipeline rather than only
        // the clear.
        let mut mesh = GpuMesh::new(&gpu);
        let (vertices, indices) = triangle();
        mesh.upload(&gpu, &vertices, &indices);

        self.window = Some(window);
        self.graphics = Some((gpu, surface, renderer, mesh));
        self.window.as_ref().unwrap().request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if Instant::now() > self.deadline {
            return self.fail(
                format!(
                    "only {} of {FRAMES_REQUIRED} frames were presented before the deadline",
                    self.outcome.presented
                ),
                event_loop,
            );
        }

        if !matches!(event, WindowEvent::RedrawRequested) {
            return;
        }

        let Some((gpu, surface, renderer, mesh)) = self.graphics.as_mut() else {
            return;
        };

        match surface.acquire(gpu) {
            Ok(frame) => {
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                renderer.render(gpu, &view, surface.framebuffer(), &self.camera, mesh, false);
                // Presenting is the step that touches the instance's registry,
                // and the step the broken version aborted on.
                frame.present();
                self.outcome.presented += 1;
            }
            Err(SurfaceLoss::Skip | SurfaceLoss::Reconfigure) => {}
            Err(SurfaceLoss::DeviceLost) => {
                return self.fail("the device was lost while presenting", event_loop)
            }
        }

        if self.outcome.presented >= FRAMES_REQUIRED {
            event_loop.exit();
        } else if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

/// One triangle, facing the default camera.
fn triangle() -> (Vec<clayspace_view::Vertex>, Vec<u32>) {
    let vertex = |position: [f32; 3]| clayspace_view::Vertex {
        position,
        normal: [0.0, 0.0, 1.0],
        color: [1.0, 1.0, 1.0],
    };
    (
        vec![
            vertex([-0.6, -0.5, 0.0]),
            vertex([0.6, -0.5, 0.0]),
            vertex([0.0, 0.7, 0.0]),
        ],
        vec![0, 1, 2],
    )
}

/// Runs on the process main thread, because this test target sets
/// `harness = false`. macOS refuses to create an event loop anywhere else.
fn main() {
    the_window_presents_real_frames();
    println!("test the_window_presents_real_frames ... ok");
}

fn the_window_presents_real_frames() {
    let required = std::env::var("CLAYSPACE_REQUIRE_WINDOW").is_ok_and(|v| v != "0");

    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(e) => {
            // No display, no window server: ordinary in a headless CI runner.
            if required {
                panic!("CLAYSPACE_REQUIRE_WINDOW is set but no event loop is available: {e}");
            }
            eprintln!("skipping the window smoke test: no event loop ({e})");
            return;
        }
    };
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SmokeApp::new(Instant::now() + Duration::from_secs(20));
    if let Err(e) = event_loop.run_app(&mut app) {
        panic!("the event loop failed: {e}");
    }

    if let Some(failure) = app.outcome.failure {
        panic!("{failure}");
    }
    assert!(
        app.outcome.presented >= FRAMES_REQUIRED,
        "the window presented {} frames, expected {FRAMES_REQUIRED}",
        app.outcome.presented
    );
}
