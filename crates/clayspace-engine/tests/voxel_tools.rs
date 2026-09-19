//! The voxel-only tools, on the layer they belong to.
//!
//! `visual_brushes` runs every tool over an SDF layer, where these four
//! correctly refuse — so "it refused for a stated reason" was the whole of
//! what was known about them. That is not the same as working. This drives
//! each on a voxel layer and asserts it changes the grid.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

/// The four, named once so the two halves cannot drift apart.
///
/// Pinçar stood here until `clay_layer_magnify_surface` gave it a field verb
/// (#201). It still reaches a grid — `voxel_brushes` drives it there, inward
/// and outward — but a tool a field layer now offers cannot be the example of
/// one a field layer refuses, and the refusal half is why this list exists.
/// Apagar replaced it: erase names `clay_voxel_erase_brush` on the grid and
/// nothing on the other three, so it is absent from a field on its own merits
/// rather than by leftover.
const VOXEL_ONLY: [ToolKind; 4] = [
    ToolKind::Raspar,
    ToolKind::Apagar,
    ToolKind::Preencher,
    ToolKind::Nudge,
];

/// A voxel layer with material already in it.
///
/// Every one of these tools reshapes what is there — scrape cuts, erase
/// hollows, fill closes cavities, smudge drags. On an empty grid each of them
/// is entitled to do nothing, so depositing first is what makes the question
/// meaningful.
fn packed() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document
        .add_voxel_layer("Voxels", 0.05)
        .expect("add a grid");

    // A ridge rather than one blob, so a tool that only acts where the surface
    // has curvature has something to bite on.
    //
    // The cavities Preencher needs are punched DELIBERATELY below, and that is
    // the correction: this fixture used to deposit at 0.9 and rely on the
    // dither to leave "a pepper of single-cell holes through the material it
    // just deposited". That pepper was the defect in #139 — every grid brush
    // wrote a porous crust because a fractional strength dithered against a
    // fixed seed — so a fixture that depended on it was measuring the bug.
    // Grid dabs are solid now, and a hole this verb is asked to close is one
    // the test made on purpose.
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
            .expect("deposit");
    }

    // Three single-cell holes inside the ridge, each enclosed by material: one
    // cell of erase at the grid's own resolution, placed well within the
    // deposit's radius. That is what `clay_voxel_sculpt_fill_cavities` closes.
    //
    // Size 0.025 on a 0.05 grid, which is one cell ACROSS: the footprint is
    // `round(2 * size / cell)` now that a grid brush's size is read as a
    // radius. At 0.05 each punch spanned two cells and stopped being the
    // single-cell pocket this verb exists to close.
    for at in [[-0.1f32, 0.0, 0.0], [0.0, 0.02, 0.0], [0.1, -0.02, 0.0]] {
        document
            .apply_stroke(
                ToolKind::Apagar,
                BrushSettings {
                    size: 0.025,
                    intensity: 1.0,
                    ..Default::default()
                },
                &[GestureSample {
                    position: at,
                    pressure: 1.0,
                    time: 0.0,
                }],
                [false; 3],
            )
            .expect("punch a cavity");
    }
    document
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
    let base = packed();
    // Each tool gets the ridge as it was made, so one tool's effect cannot
    // explain away another's.
    let mut inert = Vec::new();
    for tool in VOXEL_ONLY {
        let mut document = packed();
        assert!(
            tool.availability(clayspace_model::LayerState::editable(
                document.active_representation()
            ))
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
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    for tool in VOXEL_ONLY {
        let refusal = tool
            .availability(clayspace_model::LayerState::editable(
                document.active_representation(),
            ))
            .expect_err(&format!("{tool:?} was offered on an SDF layer"));
        assert!(
            format!("{refusal}").contains("voxel"),
            "{tool:?} refuses without naming why: {refusal}"
        );
    }
}
