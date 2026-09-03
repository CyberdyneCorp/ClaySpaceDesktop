//! Input to visible, for a stroke across the reference scene.

use std::time::Instant;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_model::{SculptModel, ToolKind};

use crate::figures::{ms, quantile, Figure};
use crate::groups::headless_gpu;
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("dab", Skip::NoHeadlessGpu);
    };
    let Ok(mut document) = Scene::Reference.build(policy.clone()) else {
        return run.skip("dab", Skip::SceneWouldNotBuild);
    };
    let mut geometry = SurfaceGeometry::new(&gpu);
    if geometry.rebuild(&gpu, &mut document).is_err() {
        return run.skip("dab", Skip::SurfaceWouldNotMesh);
    }

    let brush = Scene::Reference.brush();
    let mut samples: Vec<f64> = Vec::new();
    for sample in Scene::Reference.stroke(24) {
        let started = Instant::now();
        if document
            .apply_stroke(ToolKind::Padrao, brush, &[sample], [false; 3])
            .is_err()
        {
            return run.skip("dab", Skip::EditRefused);
        }
        if geometry.sync(&gpu, &mut document).is_err() {
            return run.skip("dab", Skip::SurfaceWouldNotMesh);
        }
        samples.push(ms(started.elapsed()));
    }
    samples.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));

    // The budget the specification states for a GPU backend. The CPU backend
    // is reported rather than failed, so the budget is only attached when an
    // accelerated backend is active.
    let accelerated = *policy.active() != clayspace_engine::claycore::Backend::Cpu;
    run.insert(
        "dab.median",
        Figure::ms(quantile(&samples, 0.5), accelerated.then_some(50.0)),
    );
    run.insert(
        "dab.p95",
        Figure::ms(quantile(&samples, 0.95), accelerated.then_some(100.0)),
    );
    // The two figures are reduced from one set of twenty-four dabs, so they
    // carry the same spread. This group takes its own quantiles rather than
    // going through `Run::timings`, which is why the spread is recorded by
    // hand here.
    run.spread("dab.median", &samples);
    run.spread("dab.p95", &samples);
}
