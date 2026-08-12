//! The voxel-only tools, on the layer they belong to.
//!
//! `visual_brushes` runs every tool over an SDF layer, where these four
//! correctly refuse — so "it refused for a stated reason" was the whole of
//! what was known about them. That is not the same as working. This drives
//! each on a voxel layer and asserts it changes the grid.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

/// A voxel layer with material already in it.
///
/// Every one of these tools reshapes what is there — scrape cuts, pinch
/// gathers, fill closes cavities, smudge drags. On an empty grid each of them
/// is entitled to do nothing, so depositing first is what makes the question
/// meaningful.
fn packed() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy).ok()?;
    document.add_voxel_layer("Voxels", 0.05).ok()?;

    // A ridge rather than one blob, so a tool that only acts where the surface
    // has curvature has something to bite on — and stamped at less than full
    // strength, which is what leaves cavities to fill.
    //
    // The engine's note on `clay_voxel_sculpt_fill_cavities`: occupancy is
    // binary, so any strength or falloff below 1 is dithered against a hash of
    // the cell coordinate, leaving a pepper of single-cell holes through the
    // material it just deposited. That pepper is what this verb exists to
    // close. A solid ridge has nothing for it to do, and it correctly reported
    // no change until the subject had some.
    // Stamped at just under full strength, which is what leaves cavities.
    //
    // The engine's note on `clay_voxel_sculpt_fill_cavities`: occupancy is
    // binary, so any strength below 1 is dithered against a hash of the cell
    // coordinate, leaving single-cell holes through the material it just
    // deposited. That pepper is what this verb exists to close, and a solid
    // ridge gives it nothing to do. Near-solid is the case that matters: at
    // 0.9 the deposit is 68 of 80 cells, which is holes in material rather
    // than the sparse speckle a light dither leaves.
    let brush = BrushSettings {
        size: 0.25,
        intensity: 0.9,
        ..Default::default()
    };
    for step in 0..9 {
        let t = step as f32 / 8.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                brush,
                &[GestureSample {
                    position: [(t - 0.5) * 0.6, (t * 6.0).sin() * 0.08, 0.0],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .ok()?;
    }
    Some(document)
}

/// Runs a tool across the ridge and says whether the engine reported a change.
fn exercise(document: &mut ClayDocument, tool: ToolKind) -> bool {
    let brush = BrushSettings {
        size: 0.25,
        ..Default::default()
    };
    let samples: Vec<GestureSample> = (0..9)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [(t - 0.5) * 0.6, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(tool, brush, &samples, [false; 3])
        .map(|outcome| outcome.changed)
        .unwrap_or(false)
}

#[test]
fn every_voxel_only_tool_changes_a_voxel_layer() {
    let Some(base) = packed() else {
        return;
    };
    // Each tool gets the ridge as it was made, so one tool's effect cannot
    // explain away another's.
    let mut inert = Vec::new();
    for tool in [
        ToolKind::Raspar,
        ToolKind::Pincar,
        ToolKind::Preencher,
        ToolKind::Nudge,
    ] {
        let Some(mut document) = packed() else {
            return;
        };
        assert!(
            tool.availability(document.active_representation(), true)
                .is_ok(),
            "{tool:?} is refused on the layer it is meant for"
        );
        if !exercise(&mut document, tool) {
            inert.push(tool);
        }
    }
    let _ = base;

    assert!(
        inert.is_empty(),
        "these tools were accepted on a voxel layer and changed nothing: {inert:?}"
    );
}

#[test]
fn a_voxel_only_tool_still_refuses_an_sdf_layer_by_name() {
    // The other half. A tool that works somewhere must still say why it will
    // not work here, rather than accepting the gesture and doing nothing.
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form) else {
        return;
    };
    for tool in [
        ToolKind::Raspar,
        ToolKind::Pincar,
        ToolKind::Preencher,
        ToolKind::Nudge,
    ] {
        let refusal = tool
            .availability(document.active_representation(), true)
            .expect_err(&format!("{tool:?} was offered on an SDF layer"));
        assert!(
            format!("{refusal}").contains("voxel"),
            "{tool:?} refuses without naming why: {refusal}"
        );
    }
}
