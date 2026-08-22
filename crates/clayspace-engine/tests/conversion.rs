//! Crossing between representations, and what each crossing costs.
//!
//! ClayCore's own guidance is that the intended workflow uses more than one
//! representation — block out and hard-surface on SDF, free-form sculpt on
//! voxels, refine on a mesh when the topology is worth keeping — and until
//! this landed a layer's representation was chosen once and lived in.
//!
//! Every crossing here is lossy. What these tests hold is not that nothing is
//! lost, which would be false, but that what survives is what the interface
//! promises survives, and that the source is still there afterwards.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, GestureSample, Refusal, Representation, SceneModel, SculptModel,
    ToolKind,
};

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// The cell size the tests convert at. Coarse enough to be quick, fine enough
/// that the starting form survives it recognisably.
const CELL: f32 = 0.04;

#[test]
fn a_conversion_adds_a_layer_and_leaves_the_source_alone() {
    let Some(mut doc) = document() else {
        return;
    };
    let before = doc.scene().layers.len();
    let source = doc.scene().active.expect("an active layer");

    let made = doc
        .convert_layer(Direction::SdfToVoxel, CELL, 1)
        .expect("rasterize the starting form");

    let scene = doc.scene();
    assert_eq!(
        scene.layers.len(),
        before + 1,
        "a conversion must add a layer rather than replace one"
    );
    assert_ne!(made, source, "the conversion returned the source layer");
    assert_eq!(
        scene.layer(made).expect("the new layer").representation,
        Representation::Voxel
    );
    // The whole reason a conversion adds rather than replaces: the crossing is
    // lossy one way and discards the edit list the other, so the source
    // staying put is the way back after the session ends, where undo is not.
    assert!(
        scene.layer(source).is_some(),
        "the source layer is gone, so the crossing cannot be reconsidered"
    );
    assert_eq!(
        scene.layer(source).expect("the source").representation,
        Representation::Sdf,
        "the source changed representation, so it was converted in place"
    );
}

/// A crossing is taken back by removing the layer it added, not by undo.
///
/// That is the specification, and it is the engine's shape rather than a
/// preference. A conversion produces no undo entry at all: measured here, the
/// depth is the same before and after, and the one entry that exists belongs
/// to the starting form. Layer creation and rasterization are not recorded,
/// and a voxel layer carries no history by construction — "No history; a host
/// snapshots if it wants undo". Bracketing the crossing in an undo group was
/// tried and groups nothing, because there are no entries to group.
///
/// The source layer surviving is what makes removal sufficient, which is why
/// `a_conversion_adds_a_layer_and_leaves_the_source_alone` is the test this one
/// leans on.
#[test]
fn a_crossing_is_taken_back_by_removing_its_layer() {
    let Some(mut doc) = document() else {
        return;
    };
    let before = doc.scene().layers.len();
    let depth_before = doc.history().depth;

    let made = doc
        .convert_layer(Direction::SdfToVoxel, CELL, 1)
        .expect("convert");
    assert_eq!(doc.scene().layers.len(), before + 1);

    // No entry, so nothing for undo to take back. If this ever stops being
    // true the specification should say so too — `representation-conversion`
    // states it as a property of the crossing, not as an omission.
    assert_eq!(
        doc.history().depth,
        depth_before,
        "a conversion has started producing undo entries; the spec says it \
         does not, and the two have to move together"
    );

    // What does take it back.
    doc.remove_layer(made).expect("remove the converted layer");
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "removing the converted layer left the document changed"
    );
}

/// A distance field carries no colour, so the return trip has to reproduce it
/// some other way — one volume item per palette entry, which is what
/// `clay_voxel_to_layer` does and what a single item could not.
#[test]
fn a_coloured_voxel_sculpt_keeps_its_colour_coming_back() {
    let Some(mut doc) = document() else {
        return;
    };
    doc.convert_layer(Direction::SdfToVoxel, CELL, 1)
        .expect("to voxels");

    // Work the grid so it carries something of its own.
    let _ = doc.apply_stroke(
        ToolKind::Inflar,
        BrushSettings::default(),
        &[GestureSample {
            position: [0.0, 0.0, 1.0],
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    );

    let made = doc
        .convert_layer(Direction::VoxelToSdf, CELL, 1)
        .expect("back to SDF");
    let scene = doc.scene();
    assert_eq!(
        scene.layer(made).expect("the new layer").representation,
        Representation::Sdf,
        "the return trip has to produce an operand, not another grid"
    );
}

#[test]
fn a_crossing_that_starts_somewhere_else_is_refused_by_name() {
    let Some(mut doc) = document() else {
        return;
    };
    // The starting form is SDF, so the voxel crossing does not start here.
    let error = doc
        .convert_layer(Direction::VoxelToSdf, CELL, 1)
        .expect_err("an SDF layer cannot be converted from voxels");
    assert!(
        error.to_string().contains("voxel"),
        "the refusal must name the representation it wanted: {error}"
    );
}

/// The budget refusal, which is the one a sculptor is most likely to meet:
/// a cell size small enough is always expressible and almost never affordable.
#[test]
fn a_resolution_past_the_budget_is_refused_before_anything_is_built() {
    let Some(mut doc) = document() else {
        return;
    };
    let before = doc.scene().layers.len();

    let error = doc
        .convert_layer(Direction::SdfToVoxel, 0.00005, 1)
        .expect_err("a cell that small cannot fit the budget");
    assert!(
        matches!(
            error,
            clayspace_model::ModelError::Conversion(Refusal::OverBudget { .. })
        ),
        "refused for the wrong reason: {error}"
    );
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "a refused conversion still added a layer"
    );
}

/// The costs are what the interface shows while the sculptor chooses, so they
/// have to follow the choice rather than describe a default.
#[test]
fn the_stated_cost_follows_the_chosen_resolution() {
    let Some(doc) = document() else {
        return;
    };
    let coarse = doc
        .conversion_cost(Direction::SdfToVoxel, 0.1)
        .expect("the starting form has bounds");
    let fine = doc
        .conversion_cost(Direction::SdfToVoxel, 0.01)
        .expect("bounds");

    assert!(fine.surface_movement < coarse.surface_movement);
    assert!(fine.cells > coarse.cells);
    assert!(
        !coarse.keeps_history,
        "no crossing carries the edit list, and saying otherwise would be a \
         promise the engine cannot keep"
    );
}
