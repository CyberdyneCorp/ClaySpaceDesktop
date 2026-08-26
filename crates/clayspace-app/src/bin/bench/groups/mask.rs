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

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
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
