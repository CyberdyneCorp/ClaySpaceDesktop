//! What taking an edit back costs, against the edit it takes back.
//!
//! Nothing measured undo at all until `undo_cost.rs`, which is how it came to
//! cost seventy times the dab it reversed. That test holds the ratio against
//! regression on one scene; this is the figure, in the baseline, where a
//! change of engine can be read off it.
//!
//! The ratio is the figure that matters. Both timings are taken moments apart
//! on the same document, so their quotient says something a machine cannot
//! move — and undo's own bound is the layer rather than the edit, which is
//! upstream, so the absolute number is expected to be large and is not the
//! news.
//!
//! # Against `undo_cost.rs`
//!
//! That test reports, on its own scene of 1045 surface bricks: a dab at 0.29 ms
//! of edit and 1.82 ms of sync over 18 keys, an undo at 12.38 ms and 56.16 ms
//! over 1045. A ratio of about 32. This group, on the reference scene's 1049
//! surface bricks, reports about 23 — the same story and the same cause, with
//! a cheaper dab behind it because the reference scene's tape is shorter than
//! ninety-six edits. Checked, and consistent: neither number is measuring
//! something the other is not.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{ModelError, SculptModel, ToolKind};
use clayspace_view::Gpu;

use crate::figures::{mean, ms, Figure, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("history", Skip::NoHeadlessGpu);
    };
    match cycles(&gpu, policy) {
        Ok(measured) => record(run, measured),
        Err(why) => run.skip("history", why),
    }
}

/// What one dab, its undo and its redo each cost, per cycle.
struct Cycles {
    edits: Vec<f64>,
    undos: Vec<f64>,
    redos: Vec<f64>,
}

/// One cycle per sample: dab, take it back, put it back, take it back again.
///
/// The document ends each cycle where it started, which is what makes the
/// samples comparable to each other rather than to a document that has grown
/// twelve dabs deeper.
fn cycles(gpu: &Gpu, policy: &BackendPolicy) -> Result<Cycles, Skip> {
    let scene = Scene::Reference;
    let mut document = scene
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let mut measured = Cycles {
        edits: Vec::new(),
        undos: Vec::new(),
        redos: Vec::new(),
    };
    for sample in scene.stroke(Record::Repeatable.samples()) {
        measured
            .edits
            .push(time(gpu, &mut document, &mut screen, |document| {
                document
                    .apply_stroke(ToolKind::Padrao, scene.brush(), &[sample], [false; 3])
                    .map(|_| ())
            })?);
        measured
            .undos
            .push(time(gpu, &mut document, &mut screen, undo)?);
        measured
            .redos
            .push(time(gpu, &mut document, &mut screen, redo)?);
        time(gpu, &mut document, &mut screen, undo)?;
    }
    Ok(measured)
}

/// One step, from the call to the surface arriving.
fn time(
    gpu: &Gpu,
    document: &mut ClayDocument,
    screen: &mut Screen,
    what: impl FnOnce(&mut ClayDocument) -> Result<(), ModelError>,
) -> Result<f64, Skip> {
    let started = Instant::now();
    what(document).map_err(|_| Skip::EditRefused)?;
    screen.refresh(gpu, document)?;
    Ok(ms(started.elapsed()))
}

/// An undo that took nothing back is a measurement of nothing, so it is a
/// refusal rather than a zero.
fn undo(document: &mut ClayDocument) -> Result<(), ModelError> {
    match document.undo() {
        Ok(true) => Ok(()),
        Ok(false) => Err(ModelError::engine("there was nothing to undo")),
        Err(e) => Err(e),
    }
}

fn redo(document: &mut ClayDocument) -> Result<(), ModelError> {
    match document.redo() {
        Ok(true) => Ok(()),
        Ok(false) => Err(ModelError::engine("there was nothing to redo")),
        Err(e) => Err(e),
    }
}

fn record(run: &mut Run, measured: Cycles) {
    // The statistic the figures beside it are reported by, so the ratio is
    // their quotient rather than a third opinion.
    let edit = mean(&measured.edits);
    let undone = mean(&measured.undos);
    run.timings("history.edit", Record::Repeatable, measured.edits);
    run.timings("history.undo", Record::Repeatable, measured.undos);
    run.timings("history.redo", Record::Repeatable, measured.redos);
    // How many times the edit an undo costs. The one figure here that means
    // the same thing on another machine.
    run.insert(
        "history.undo_ratio",
        Figure::ratio(undone / edit.max(f64::MIN_POSITIVE), None, 1.5),
    );
}
