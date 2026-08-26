//! The two things a sculptor builds rather than strokes: a rig, and a curve.
//!
//! Neither is a brush and neither is a `LayerOperation`. A rig is a tree of
//! spheres whose skin the engine rewrites on every edit — so the cost of
//! authoring one is the cost of nine skins, not of nine tree insertions — and
//! a curve is a set of points that stay where they are put, swept into a form
//! that is re-swept as they move.
//!
//! Both are one-shots: the second rig on a layer replaces the first, and a
//! curve applied is a curve taken down.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{ArmatureModel, CurveModel, SkinSettings};
use clayspace_view::Gpu;

use crate::figures::{ms, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

/// The rig `visual_armature` draws: a torso, a head, two arms and two legs,
/// nine drags that come out as twelve spheres once the mirror has had them.
const RIG: [([f32; 3], [f32; 3]); 8] = [
    // (where the new sphere goes, where its parent is)
    ([0.0, -0.1, 0.0], [0.0, -0.5, 0.0]),
    ([0.0, 0.3, 0.0], [0.0, -0.1, 0.0]),
    ([0.0, 0.7, 0.0], [0.0, 0.3, 0.0]),
    ([0.45, 0.3, 0.0], [0.0, 0.3, 0.0]),
    ([0.8, 0.05, 0.0], [0.45, 0.3, 0.0]),
    ([0.22, -0.9, 0.0], [0.0, -0.5, 0.0]),
    ([0.25, -1.35, 0.0], [0.22, -0.9, 0.0]),
    ([0.0, 1.0, 0.0], [0.0, 0.7, 0.0]),
];

/// A curve laid across the front of the form, clear of it.
const CURVE: [([f32; 3], f32); 3] = [
    ([-0.9, 1.4, 0.0], 0.12),
    ([0.0, 1.7, 0.0], 0.16),
    ([0.9, 1.4, 0.0], 0.10),
];

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("authoring", Skip::NoHeadlessGpu);
    };
    one(&gpu, policy, run, "authoring.armature", author_a_rig);
    one(&gpu, policy, run, "authoring.skin", reskin_a_rig);
    one(&gpu, policy, run, "authoring.curve", lay_a_curve);
}

/// Takes one measurement's samples, on a document rebuilt for each.
fn one(
    gpu: &Gpu,
    policy: &BackendPolicy,
    run: &mut Run,
    prefix: &str,
    what: fn(&mut ClayDocument) -> Result<(), Skip>,
) {
    if !run.wants_group(prefix) {
        return;
    }
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| time(gpu, policy, what))
        .collect();
    match samples {
        Ok(samples) => run.timings(prefix, Record::OneShot, samples),
        Err(why) => run.skip(prefix, why),
    }
}

fn time(
    gpu: &Gpu,
    policy: &BackendPolicy,
    what: fn(&mut ClayDocument) -> Result<(), Skip>,
) -> Result<f64, Skip> {
    let mut document = Scene::Reference
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let started = Instant::now();
    what(&mut document)?;
    screen.refresh(gpu, &mut document)?;
    Ok(ms(started.elapsed()))
}

/// Building the rig from nothing, mirrored, as the pointer would.
fn author_a_rig(document: &mut ClayDocument) -> Result<(), Skip> {
    document
        .begin_armature([0.0, -0.5, 0.0], 0.2)
        .map_err(|_| Skip::EditRefused)?;
    for (index, (position, _)) in RIG.iter().enumerate() {
        // Under the sphere before it, which is the chain the drags above make.
        let parent = index as clayspace_model::NodeIndex;
        document
            .add_zsphere(parent, *position, 0.18, true)
            .map_err(|_| Skip::EditRefused)?;
    }
    Ok(())
}

/// Changing the skin's thickness, which rewrites every cone in the rig.
fn reskin_a_rig(document: &mut ClayDocument) -> Result<(), Skip> {
    author_a_rig(document)?;
    document
        .set_skin(SkinSettings { thickness: 1.3 })
        .map_err(|_| Skip::EditRefused)
}

/// Laying a curve and leaving the swept form in the layer.
fn lay_a_curve(document: &mut ClayDocument) -> Result<(), Skip> {
    document.begin_curve();
    for (at, radius) in CURVE {
        document
            .add_curve_point(at, radius)
            .map_err(|_| Skip::EditRefused)?;
    }
    document.apply_curve().map_err(|_| Skip::EditRefused)
}
