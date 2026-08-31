//! What a mask costs the edit it gates.
//!
//! Painting one is `brush.*.mascara`, with the other brushes, because that is
//! what it is: a stroke that freezes rather than displaces. What is not there
//! is what a frozen region costs *everything else* — the gate is consulted per
//! sample on every verb, and a gate that has become expensive shows up as
//! every tool being slower rather than as anything about masking.
//!
//! So the figure is a ratio: the same dab on the same scene, once with a
//! frozen region and once without. A ratio survives a change of machine, which
//! is what makes it worth recording next to two absolute timings that do not
//! agree across one.
//!
//! `mask.outline` is the other thing masking costs, and it is an absolute
//! rather than a ratio because it is a whole gesture rather than a per-sample
//! gate. It is here rather than with the brushes for the same reason `Trim` is
//! not there: the gesture is a shape drawn on the view frame, not a stroke
//! across the surface, and it is bounded by the subtool rather than by the
//! brush — so its cost follows the *form's* size, which is the thing worth
//! watching.
//!
//! One figure for both drawn gestures, because there is only one thing to
//! measure: a lasso and a rectangle differ in how the pointer builds the
//! shape, and by the time one reaches the document it is a list of points
//! either way.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, GestureSample, MaskModel, MaskOutline, OutlineFrame, OutlineMode, SculptModel,
    ToolKind,
};
use clayspace_view::Gpu;

use crate::figures::{mean, ms, Figure, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("mask", Skip::NoHeadlessGpu);
    };
    let ungated = match dabs(&gpu, policy, false) {
        Ok(samples) => samples,
        Err(why) => return run.skip("mask", why),
    };
    let gated = match dabs(&gpu, policy, true) {
        Ok(samples) => samples,
        Err(why) => return run.skip("mask", why),
    };

    run.timings("mask.ungated", Record::Repeatable, ungated.clone());
    run.timings("mask.gated", Record::Repeatable, gated.clone());
    // The same statistic the two figures beside it are reported by, so the
    // ratio is their quotient and not a third opinion.
    let ratio = mean(&gated) / mean(&ungated).max(f64::MIN_POSITIVE);
    // No budget: what a gate should cost is not something the specification
    // states, and a number invented here would be a promise nobody made.
    run.insert("mask.gated_ratio", Figure::ratio(ratio, None, 1.5));

    match outlines(policy) {
        Ok(samples) => run.timings("mask.outline", Record::Repeatable, samples),
        Err(why) => run.skip("mask.outline", why),
    }
}

/// An outline thrown around the whole of the reference form, several times.
///
/// The whole of it deliberately: the region is bounded by the subtool's extent
/// and swept through it, so the worst gesture there is the one that encloses
/// everything, and a figure taken from a small outline would say nothing about
/// the one a sculptor waits on.
fn outlines(policy: &BackendPolicy) -> Result<Vec<f64>, Skip> {
    let mut document = Scene::Reference
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;

    // Looking down -z, which is where the reference scene's own stroke is laid
    // from, with the outline covering the form and a margin either side.
    let frame = OutlineFrame {
        origin: [0.0, 0.0, 0.0],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        scale: [1.0, 1.0],
    };
    let half = 1.4;
    let outline = vec![[-half, -half], [half, -half], [half, half], [-half, half]];

    (0..Record::Repeatable.samples())
        .map(|at| {
            // Alternating, so each measured gesture has something to do: a
            // second freeze over an already frozen region writes the same
            // cells and would measure the second-cheapest thing an outline does.
            let mode = if at % 2 == 0 {
                OutlineMode::Freeze
            } else {
                OutlineMode::Thaw
            };
            let outline = MaskOutline {
                outline: outline.clone(),
                frame,
                mode,
            };
            let started = Instant::now();
            document
                .apply_outline(&outline)
                .map_err(|_| Skip::EditRefused)?;
            Ok(ms(started.elapsed()))
        })
        .collect()
}

/// A stroke's worth of dabs on the reference scene, with or without a frozen
/// region under them.
fn dabs(gpu: &Gpu, policy: &BackendPolicy, frozen: bool) -> Result<Vec<f64>, Skip> {
    let scene = Scene::Reference;
    let mut document = scene
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    if frozen {
        paint(&mut document, &scene)?;
        screen.refresh(gpu, &mut document)?;
    }

    scene
        .stroke(Record::Repeatable.samples())
        .into_iter()
        .map(|sample| {
            let started = Instant::now();
            document
                .apply_stroke(ToolKind::Padrao, scene.brush(), &[sample], [false; 3])
                .map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, &mut document)?;
            Ok(ms(started.elapsed()))
        })
        .collect()
}

/// Freezes a band across the subject, where the dabs are about to land.
///
/// Over the stroke rather than beside it: a mask the edit never reaches is a
/// mask the gate can answer without reading, which measures nothing.
fn paint(document: &mut ClayDocument, scene: &Scene) -> Result<(), Skip> {
    let brush = BrushSettings {
        size: 0.3,
        intensity: 1.0,
        ..scene.brush()
    };
    let samples: Vec<GestureSample> = scene.stroke(8);
    document
        .apply_stroke(ToolKind::Mascara, brush, &samples, [false; 3])
        .map(|_| ())
        .map_err(|_| Skip::EditRefused)
}
