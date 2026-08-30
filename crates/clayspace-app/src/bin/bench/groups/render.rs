//! Rendering the reference scene with nothing being edited.

use std::time::Instant;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_model::SculptModel;
use clayspace_view::{Camera, OffscreenTarget, Renderer};

use crate::figures::{ms, quantile, Figure};
use crate::groups::{headless_gpu, VIEWPORT};
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("frame", Skip::NoHeadlessGpu);
    };
    let Ok(mut document) = Scene::Reference.build(policy.clone()) else {
        return run.skip("frame", Skip::SceneWouldNotBuild);
    };
    let mut geometry = SurfaceGeometry::new(&gpu);
    if geometry.rebuild(&gpu, &mut document).is_err() {
        return run.skip("frame", Skip::SurfaceWouldNotMesh);
    }

    let renderer = Renderer::new(&gpu, OffscreenTarget::FORMAT);
    let target = OffscreenTarget::new(&gpu, VIEWPORT.0, VIEWPORT.1);
    let mut camera = Camera::default();
    match document.bounds() {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }

    let mut frames: Vec<f64> = Vec::new();
    for i in 0..32 {
        // Orbiting, because a static frame does not exercise what a moving
        // camera does to culling and upload.
        camera.orbit(0.02, 0.0);
        let started = Instant::now();
        let _ = target.capture(&gpu, &renderer, &camera, geometry.mesh(), false);
        let elapsed = ms(started.elapsed());
        // The first few include pipeline and buffer warmup.
        if i >= 4 {
            frames.push(elapsed);
        }
    }
    frames.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));

    // 60 fps is 16.7 ms. This is an offscreen capture including a readback,
    // which a presenting frame does not pay, so it is reported without a
    // budget rather than judged against one that does not describe it.
    run.insert("frame.median", Figure::ms(quantile(&frames, 0.5), None));
    run.insert("frame.p95", Figure::ms(quantile(&frames, 0.95), None));
}

/// What each viewport size costs, per pass.
///
/// The `frame.*` figures above measure a capture — a render *and* a readback
/// into host memory — at one size, which is the right shape for "can this
/// machine draw the reference scene" and the wrong shape for "what does the
/// occlusion pass cost". At 4K the readback alone moves 33 MB across the bus
/// and drowns everything the renderer did.
///
/// So these render and wait, without reading back, at the sizes a sculptor
/// actually works at; and where the adapter will report its own clock, they
/// take the device's per-pass time as well. That is the figure a change to the
/// occlusion path has to be argued against.
pub fn measure_passes(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("render", Skip::NoHeadlessGpu);
    };
    let Ok(mut document) = Scene::Reference.build(policy.clone()) else {
        return run.skip("render", Skip::SceneWouldNotBuild);
    };
    let mut geometry = SurfaceGeometry::new(&gpu);
    if geometry.rebuild(&gpu, &mut document).is_err() {
        return run.skip("render", Skip::SurfaceWouldNotMesh);
    }

    let mut renderer = Renderer::new(&gpu, OffscreenTarget::FORMAT);
    if !renderer.gpu_timing_available() {
        run.skip("render.gpu", Skip::NoGpuTimestamps);
    }
    let ceiling = gpu.device.limits().max_texture_dimension_2d;

    for (name, width, height) in SIZES {
        if width > ceiling || height > ceiling {
            run.skip(format!("render.{name}"), Skip::ViewportTooLarge);
            continue;
        }
        let target = OffscreenTarget::new(&gpu, width, height);
        let mut camera = Camera::default();
        match document.bounds() {
            Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
            None => camera.frame_default(),
        }

        for occlusion in [true, false] {
            renderer.set_occlusion(occlusion);
            let frames = sweep(&gpu, &renderer, &target, &mut camera, geometry.mesh());
            let leaf = if occlusion { "frame" } else { "ao_off.frame" };
            run.insert(
                format!("render.{name}.{leaf}.median"),
                Figure::ms(quantile(&frames, 0.5), None),
            );
            run.insert(
                format!("render.{name}.{leaf}.p95"),
                Figure::ms(quantile(&frames, 0.95), None),
            );
        }

        // With occlusion back on, which is what the pass figures describe.
        renderer.set_occlusion(true);
        let _ = sweep(&gpu, &renderer, &target, &mut camera, geometry.mesh());
        if let Some(timing) = renderer.gpu_timing() {
            for (pass, ms) in timing.measured() {
                // The pass labels carry spaces; the figure names do not.
                let leaf = pass.label().replace(' ', "_");
                run.insert(
                    format!("render.{name}.gpu.{leaf}"),
                    // A pass time is tens of microseconds on a fast card, well
                    // under the millisecond floor a scheduling-noise figure
                    // uses — this is the device's own clock, not a wall clock,
                    // so there is no scheduling in it to be noise.
                    Figure {
                        noise_floor: 0.02,
                        ..Figure::ms(ms as f64, None)
                    },
                );
            }
        }

        // The geometry the frame asked for, which is what a culling or
        // batching change is supposed to move.
        let stats = renderer.frame_stats();
        run.insert(
            format!("render.{name}.draws"),
            Figure::count(stats.draw_calls as f64),
        );
        run.insert(
            format!("render.{name}.triangles"),
            Figure::count(stats.triangles as f64),
        );
    }
}

/// The sizes a sculptor works at: a laptop panel, a 1440p monitor and 4K.
const SIZES: [(&str, u32, u32); 3] = [
    ("1080p", 1920, 1080),
    ("1440p", 2560, 1440),
    ("2160p", 3840, 2160),
];

/// Renders a short orbit and returns each frame's wall time, sorted.
///
/// Waits on the device rather than reading the image back: the question is
/// what drawing costs, and a readback is not drawing. The first frames are
/// dropped for the reason the capture sweep drops them — a cold pipeline is
/// not the steady state.
fn sweep(
    gpu: &clayspace_view::Gpu,
    renderer: &Renderer,
    target: &OffscreenTarget,
    camera: &mut Camera,
    mesh: &clayspace_view::GpuMesh,
) -> Vec<f64> {
    let mut frames = Vec::new();
    for i in 0..20 {
        camera.orbit(0.02, 0.0);
        let started = Instant::now();
        renderer.render(
            gpu,
            target.view(),
            target.framebuffer(),
            camera,
            mesh,
            false,
        );
        // Submission returns before the device has drawn anything, so a frame
        // timed without this measures the encoder.
        let _ = gpu.device.poll(wgpu::Maintain::Wait);
        let elapsed = ms(started.elapsed());
        if i >= 6 {
            frames.push(elapsed);
        }
    }
    frames.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
    frames
}
