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
