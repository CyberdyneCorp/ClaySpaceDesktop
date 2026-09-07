//! What a shape drawn on the view costs to cut with.
//!
//! Not with the brushes, and for the reason `mask.outline` is not: the gesture
//! is a shape on the view frame rather than a stroke across the surface, and
//! what it costs follows the *form's* size rather than the brush's — a cut is
//! swept from the layer's own bounds and passes all the way through, so the
//! work is the region it crosses. `brush.sdf.trim` stays skipped as
//! `NoGestureForTool` because that is exactly true of the brush harness, which
//! synthesises strokes; it is not a claim that the tool is unmeasurable.
//!
//! Two figures rather than one, unlike the mask's, because a line and a lasso
//! take **different routes through the engine**: a line is closed against the
//! frame's bounds through `clay_cut_polygon_from_open_curve` and a lasso is
//! closed as it was drawn through `clay_cut_polygon_from_curve`. A rectangle
//! is not measured separately — it is the engine's own `CLAY_CUT_RECT` with no
//! outline to tessellate, so it is the cheap case and the two here bound it.
//!
//! Each sample is undone before the next, outside the clock. A cut is
//! destructive: repeating one on the form it already opened would measure a
//! smaller form every time, and the figure would fall through the run for a
//! reason that has nothing to do with the code.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::BackendPolicy;
use clayspace_model::{CutGesture, CutModel, DrawnCut, OutlineFrame, SculptModel};

use crate::figures::{ms, Record};
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    for (name, gesture) in [
        ("cut.line", CutGesture::Line),
        ("cut.lasso", CutGesture::Lasso),
    ] {
        match cuts(policy, gesture) {
            Ok(samples) => run.timings(name, Record::Repeatable, samples),
            Err(why) => run.skip(name, why),
        }
    }
}

/// Looking down -z, where the reference scene's own stroke is laid from.
fn frame() -> OutlineFrame {
    OutlineFrame {
        origin: [0.0; 3],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        scale: [1.0, 1.0],
    }
}

/// The gesture, across the whole of the reference form.
///
/// The whole of it deliberately, as the mask's outline is: the shape is swept
/// through the subtool's extent, so the gesture a sculptor waits on is the one
/// that crosses everything, and a figure taken from a small cut would say
/// nothing about it.
fn track(gesture: CutGesture) -> Vec<[f32; 2]> {
    let half = 1.4;
    match gesture {
        // A slight lean, so the cut is not axis-aligned and the polygon has
        // something to tessellate.
        CutGesture::Line => vec![[-half, -0.2], [half, 0.2]],
        // Wound clockwise, which is the winding that removes what it
        // encloses — the expensive half, since the other one keeps a
        // region and drops the rest.
        _ => (0..24)
            .map(|step| {
                let angle = -(step as f32) / 24.0 * std::f32::consts::TAU;
                [angle.cos() * half, angle.sin() * half]
            })
            .collect(),
    }
}

fn cuts(policy: &BackendPolicy, gesture: CutGesture) -> Result<Vec<f64>, Skip> {
    let mut document = Scene::Reference
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let cut = DrawnCut {
        track: track(gesture),
        frame: frame(),
        gesture,
    };

    (0..Record::Repeatable.samples())
        .map(|_| {
            let started = Instant::now();
            document.apply_cut(&cut).map_err(|_| Skip::EditRefused)?;
            let took = ms(started.elapsed());
            // Off the clock, and asserted rather than ignored: a cut that did
            // not come back leaves every later sample cutting a form that is
            // already open, which is the drift this exists to avoid.
            if !document.undo().map_err(|_| Skip::EditRefused)? {
                return Err(Skip::EditRefused);
            }
            Ok(took)
        })
        .collect()
}
