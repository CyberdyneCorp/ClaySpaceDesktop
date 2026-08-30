//! What every brush costs, on every representation it exists on.
//!
//! Derived rather than listed: the loop is `Representation::ALL` against
//! `ToolKind::for_representation`, which is the same table the shelf presents
//! from. A tool added to the shelf is a tool measured, with no second list to
//! forget.
//!
//! Three kinds of gesture, because the tools genuinely differ:
//!
//! - a stamping tool deposits at each position, so a segment is one sample;
//! - a path-driven tool is told *from where to where*, so a segment carries
//!   the position it started from — one position is a gesture of length zero
//!   and moves nothing;
//! - a region-based tool bakes the region a gesture covered into a volume and
//!   replaces it, which does not decompose into segments at all. Applied to
//!   each segment in turn it stacks a replacement per segment over overlapping
//!   ground, and the seams read as a crumbling patch.
//!
//! # Against `just segments`
//!
//! `visual_brushes::no_brush_stalls_the_stroke` prints a worst-segment cost
//! for the same tools and does not agree with these figures. Checked, three
//! reasons, none of them a fault here:
//!
//! - it sculpts its own harness subject rather than the reference scene;
//! - the region-based four read 0.0 ms there because they do not preview while
//!   the pointer moves — they land on pointer-up, which that test does not
//!   time and this does. Their real cost is the several hundred milliseconds
//!   below;
//! - it reports the *worst* segment and these report a median and a 95th
//!   percentile, so a single warm-up segment moves its number and not these.
//!
//! Two measurements of different things, both worth having. This one is the
//! record.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{GestureSample, Representation, SculptModel, ToolKind};
use clayspace_view::Gpu;

use crate::figures::{ms, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

/// How many segments a live stroke is delivered in.
const SEGMENTS: usize = 12;

/// The symmetry a sculptor actually has on.
///
/// The application starts with X mirrored, and a mirrored stroke edits two
/// patches rather than one — measured on the reference scene, better than
/// three times the keys. A figure taken without it describes a setting nobody
/// works in, and a regression that only reaches the mirror path would not
/// move it at all.
///
/// `dab.*` is the unmirrored one and stays that way: it is the specification's
/// budget figure and its recorded history goes back several engine releases.
/// So the two are deliberately not the same measurement, and neither is
/// wrong.
const SYMMETRY: [bool; 3] = [true, false, false];

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("brush", Skip::NoHeadlessGpu);
    };
    for representation in Representation::ALL {
        for tool in ToolKind::for_representation(representation) {
            let prefix = name(representation, tool);
            if !run.wants_group(&prefix) {
                continue;
            }
            // Trim's gesture is a shape drawn on the view frame, resolved
            // into a prism that cuts through — not a stroke across the
            // surface. Driving it as one would time a tool doing something
            // else, so it is stated as uncovered instead.
            if !tool.is_stroke_tool() {
                run.skip(prefix, Skip::NoGestureForTool);
                continue;
            }
            let record = record_for(representation, tool);
            match measure_tool(&gpu, policy, representation, tool) {
                Ok(samples) => run.timings(&prefix, record, samples),
                Err(why) => run.skip(prefix, why),
            }
        }
    }
}

/// `brush.sdf.padrao`.
///
/// From the variant names rather than from `label()`: a label is the interface
/// text, which is translated and may change wording, and a figure's name has
/// to be the same string in next year's baseline.
fn name(representation: Representation, tool: ToolKind) -> String {
    format!("brush.{representation:?}.{tool:?}").to_lowercase()
}

/// Tools that land once are rebuilt between samples; everything else is dabbed
/// onto the document it is already on.
///
/// Asked of the representation as well as of the tool, because one of them
/// depends on it: a drag on a grid is destructive and does not compose, so
/// dabbing it repeatedly onto the same document times the second grab's
/// resampling of the first rather than a drag.
fn record_for(representation: Representation, tool: ToolKind) -> Record {
    if tool.holds_the_whole_gesture(representation) {
        Record::OneShot
    } else {
        Record::Repeatable
    }
}

/// The timings for one tool on one representation.
fn measure_tool(
    gpu: &Gpu,
    policy: &BackendPolicy,
    representation: Representation,
    tool: ToolKind,
) -> Result<Vec<f64>, Skip> {
    let scene = Scene::for_representation(representation);
    if tool.holds_the_whole_gesture(representation) {
        // The document is rebuilt between samples: the second replacement of a
        // region is not the first, and timing it as though it were measures a
        // region that has already been flattened.
        return (0..Record::OneShot.samples())
            .map(|_| {
                let (mut document, mut screen) = arrange(gpu, policy, scene)?;
                let path = scene.stroke(SEGMENTS);
                time(gpu, &mut document, &mut screen, tool, scene, &path)
            })
            .collect();
    }

    let (mut document, mut screen) = arrange(gpu, policy, scene)?;
    let path = scene.stroke(Record::Repeatable.samples() + 1);
    let segments: Vec<Vec<GestureSample>> = if tool.is_path_driven() {
        path.windows(2).map(|pair| pair.to_vec()).collect()
    } else {
        path.iter().map(|sample| vec![*sample]).collect()
    };
    segments
        .iter()
        .map(|segment| time(gpu, &mut document, &mut screen, tool, scene, segment))
        .collect()
}

/// A document of this scene, meshed and uploaded, with nothing pending.
fn arrange(
    gpu: &Gpu,
    policy: &BackendPolicy,
    scene: Scene,
) -> Result<(ClayDocument, Screen), Skip> {
    let mut document = scene
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;
    Ok((document, screen))
}

/// One application, from the edit to the surface arriving.
fn time(
    gpu: &Gpu,
    document: &mut ClayDocument,
    screen: &mut Screen,
    tool: ToolKind,
    scene: Scene,
    segment: &[GestureSample],
) -> Result<f64, Skip> {
    let started = Instant::now();
    document
        .apply_stroke(tool, scene.brush(), segment, SYMMETRY)
        .map_err(|_| Skip::EditRefused)?;
    screen.refresh(gpu, document)?;
    Ok(ms(started.elapsed()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Every pair the shelf can present is a pair this group will report on —
    /// with a figure where it can be driven, and a stated reason where it
    /// cannot. The count is the verb table's own, so a tool added to the table
    /// arrives here without anything being edited.
    #[test]
    fn every_pair_the_shelf_offers_is_named() {
        let mut names = BTreeSet::new();
        let mut pairs = 0;
        for representation in Representation::ALL {
            for tool in ToolKind::for_representation(representation) {
                pairs += 1;
                names.insert(name(representation, tool));
            }
        }
        assert_eq!(names.len(), pairs, "two pairs share a figure name");
        assert!(pairs > 0, "the verb table offers nothing at all");
    }

    #[test]
    fn a_figure_name_is_lowercase_and_dotted() {
        let name = name(Representation::Sdf, ToolKind::Padrao);
        assert_eq!(name, "brush.sdf.padrao");
    }

    #[test]
    fn the_tools_that_land_once_are_rebuilt_between_samples() {
        for representation in Representation::ALL {
            assert_eq!(
                record_for(representation, ToolKind::Suavizar),
                Record::OneShot
            );
            assert_eq!(
                record_for(representation, ToolKind::Padrao),
                Record::Repeatable
            );
        }
        // The one that depends on what is being sculpted: a drag composes on a
        // field and on a mesh and does not on a grid.
        assert_eq!(
            record_for(Representation::Voxel, ToolKind::Mover),
            Record::OneShot
        );
        assert_eq!(
            record_for(Representation::Sdf, ToolKind::Mover),
            Record::Repeatable
        );
    }
}
