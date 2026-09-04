//! How much a dab costs after the document has been worked on.
//!
//! The most important number here, and the one a bare-sphere benchmark misses
//! entirely. The bricks a dab re-meshes do not change as a document is
//! sculpted — measured, they stay at 125 from the first dab to the two
//! hundredth — but the cost of *evaluating* each of them grows with the number
//! of nodes in the layer's tape. So the application gets slower the more it is
//! used, linearly and without bound, and nothing about the edit itself says so.
//!
//! The engine's answer is consolidation, which collapses a layer's tape into a
//! baked volume; the specification requires it never run unasked. This figure
//! is what should drive offering it.

use std::time::Instant;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{GestureSample, SculptModel, ToolKind};
use clayspace_view::Gpu;

use crate::figures::{ms, quantile, Figure};
use crate::groups::headless_gpu;
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("tape", Skip::NoHeadlessGpu);
    };

    let mut points = Vec::new();
    for prior in [0usize, 96] {
        match dab_after(&gpu, policy, prior) {
            Ok(samples) => points.push(samples),
            Err(why) => return run.skip("tape", why),
        }
    }

    let [fresh_samples, worked_samples] = &points[..] else {
        return run.skip("tape", Skip::EditRefused);
    };
    let (fresh, worked) = (median(fresh_samples), median(worked_samples));
    run.insert("tape.dab_on_fresh", Figure::ms(fresh, None));
    run.spread("tape.dab_on_fresh", fresh_samples);
    run.insert("tape.dab_after_96_edits", Figure::ms(worked, None));
    run.spread("tape.dab_after_96_edits", worked_samples);
    // How much the same edit costs once the document has been used. Budgeted
    // at 5x, which is roughly where it sits today: the point is to notice the
    // slope changing, not to pretend it is flat.
    run.insert(
        "tape.growth",
        Figure::ratio(worked / fresh.max(f64::MIN_POSITIVE), Some(5.0), 1.3),
    );
}

/// The middle of a set of samples, sorted here rather than by the caller.
fn median(samples: &[f64]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
    quantile(&sorted, 0.5)
}

/// What each of twelve dabs cost on a document that has already had `prior`.
///
/// The samples rather than their middle: the ratio below wants one number, and
/// the baseline wants to be able to say how far apart the twelve were.
fn dab_after(gpu: &Gpu, policy: &BackendPolicy, prior: usize) -> Result<Vec<f64>, Skip> {
    let mut document = ClayDocument::new(policy.clone()).map_err(|_| Skip::SceneWouldNotBuild)?;
    document
        .add_starting_sphere(1.0)
        .map_err(|_| Skip::SceneWouldNotBuild)?;

    let brush = Scene::probe_brush();
    for i in 0..prior {
        let t = i as f32 / prior.max(1) as f32;
        let angle = (t - 0.5) * 1.4;
        let (s, c) = angle.sin_cos();
        document
            .apply_stroke(
                ToolKind::Padrao,
                brush,
                &[GestureSample {
                    position: [s * 1.01, (t - 0.5) * 0.6, c * 1.01],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .map_err(|_| Skip::EditRefused)?;
    }
    // Nothing pending, so what follows measures the dab rather than the
    // document's own construction.
    document.take_dirty_keys();

    let mut geometry = SurfaceGeometry::new(gpu);
    geometry
        .rebuild(gpu, &mut document)
        .map_err(|_| Skip::SurfaceWouldNotMesh)?;

    let mut times = Vec::new();
    for sample in Scene::Reference.stroke(12) {
        let started = Instant::now();
        document
            .apply_stroke(ToolKind::Padrao, brush, &[sample], [false; 3])
            .map_err(|_| Skip::EditRefused)?;
        geometry
            .sync(gpu, &mut document)
            .map_err(|_| Skip::SurfaceWouldNotMesh)?;
        times.push(ms(started.elapsed()));
    }
    Ok(times)
}
