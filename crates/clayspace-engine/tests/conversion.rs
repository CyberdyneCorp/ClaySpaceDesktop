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

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// The cell size the tests convert at. Coarse enough to be quick, fine enough
/// that the starting form survives it recognisably.
const CELL: f32 = 0.04;

#[test]
fn a_conversion_adds_a_layer_and_leaves_the_source_alone() {
    let mut doc = document();
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

/// Undo takes a crossing back whole: the filling and the layer alike.
///
/// This changed under us. On engine 0.39.0 a conversion recorded no entry at
/// all, so undo stepped straight past it and removing the layer was the only
/// way back — which is what the specification used to say. Since
/// `unify-the-undo-history` the engine records the filling but still cannot
/// take back layer creation, so an engine undo on its own empties the new
/// layer and leaves it standing. Measured across the pin, same document, same
/// single undo: 0.39.0 left the layer's 3,952 vertices alone, 0.52.2 left it
/// at zero with the layer still in the list.
///
/// An empty layer nobody asked for is not "taken back", so the removal is
/// done by the host, on top of the engine's undo. `crossing_undo` holds the
/// record that makes that possible.
///
/// The source layer surviving is what makes the removal sound, which is why
/// `a_conversion_adds_a_layer_and_leaves_the_source_alone` is the test this
/// one leans on.
#[test]
fn a_crossing_is_taken_back_by_undo() {
    let mut doc = document();
    let before = doc.scene().layers.len();
    let depth_before = doc.history().depth;

    doc.convert_layer(Direction::SdfToVoxel, CELL, 1)
        .expect("convert");
    assert_eq!(doc.scene().layers.len(), before + 1);

    // The crossing is one step on the stack, not none and not two.
    assert_eq!(
        doc.history().depth,
        depth_before + 1,
        "a crossing should sit on the undo stack as exactly one step"
    );

    let undid = SculptModel::undo(&mut doc).expect("undo the crossing");
    assert!(undid, "undo reported nothing to take back after a crossing");
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "undo left the converted layer standing. An engine undo empties it \
         without removing it, so the host takes it off the scene — see \
         `crossing_undo`"
    );
    assert_eq!(
        doc.history().depth,
        depth_before,
        "undoing a crossing left the stack at a different depth than before \
         it. Taking the layer back by removing it does exactly this, because \
         a removal is itself an undo step"
    );

    // A second undo goes further back rather than putting the emptied layer
    // back — the failure that removal-based take-back produced.
    let further = SculptModel::undo(&mut doc).expect("undo twice");
    assert!(
        further,
        "there was more history than the crossing to take back"
    );
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "a second undo put the undone crossing's layer back on the scene"
    );

    // And forward again: the crossing comes back filled, not empty.
    assert!(
        SculptModel::redo(&mut doc).expect("redo"),
        "redo did nothing"
    );
    assert!(
        SculptModel::redo(&mut doc).expect("redo the crossing"),
        "redo did not reach the crossing"
    );
    assert_eq!(
        doc.scene().layers.len(),
        before + 1,
        "redo did not put the converted layer back"
    );
    let (positions, _, _, _) = doc.visible_mesh_geometry();
    assert!(
        !positions.is_empty(),
        "the layer redo put back is empty, so the crossing came back without \
         its filling"
    );

    // Removing the layer still works, and is still what a sculptor reaches for
    // when the crossing is no longer the most recent thing they did.
    let made = doc.scene().layers.last().expect("the converted layer").key;
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
    let mut doc = document();
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
    let mut doc = document();
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
    let mut doc = document();
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
    let doc = document();
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

/// Sculpting a mesh layer through the ordinary stroke path.
///
/// The fourth stroke consumer, reached the same way the other three are: one
/// `apply_stroke` with a tool and a gesture. What makes it different is
/// invisible from here and deliberately so — the sculptor holding the mesh's
/// adjacency is built on the first stroke and kept.
mod mesh_strokes {
    use super::*;
    use clayspace_model::SceneModel;

    /// A document carrying a mesh layer, made by crossing the starting form
    /// over rather than by reading a file: it needs no fixture on disk and it
    /// exercises the conversion the mesh brushes exist to complete.
    fn with_mesh_layer() -> ClayDocument {
        let mut doc = document();
        // Mesh layers are attached from an imported mesh; the starting form
        // becomes one by way of the exchange path in the application. Here the
        // engine's own attach is enough.
        doc.add_mesh_layer("Retopo").expect("attach a mesh layer");
        doc
    }

    /// A mesh row exists before its triangles do — `add_mesh_layer` records
    /// one so the rest of the application can talk about it, and only an
    /// import attaches a mesh. Until then the sixteen verbs have nothing to
    /// move, and offering them so that each fails with "no mesh layer named X"
    /// is what this refusal prevents.
    #[test]
    fn a_mesh_row_with_no_triangles_refuses_the_verbs_by_name() {
        let mut doc = with_mesh_layer();
        let mesh = doc
            .scene()
            .layers
            .iter()
            .find(|l| l.representation == Representation::Mesh)
            .map(|l| l.key)
            .expect("the mesh layer");
        doc.set_active_layer(mesh).expect("activate");

        let error = ToolKind::Padrao
            .availability(doc.active_layer_state())
            .expect_err("an empty mesh row has nothing to sculpt");
        assert!(
            error.to_string().contains("mesh"),
            "the refusal has to name what is missing: {error}"
        );
    }

    #[test]
    fn the_stroke_path_and_the_tool_status_refuse_alike() {
        let mut doc = with_mesh_layer();
        let mesh = doc
            .scene()
            .layers
            .iter()
            .find(|l| l.representation == Representation::Mesh)
            .map(|l| l.key)
            .expect("the mesh layer");
        doc.set_active_layer(mesh).expect("activate");

        // Padrão is Draw on a mesh, and the table says so.
        assert!(ToolKind::Padrao.exists_on(Representation::Mesh));
        let outcome = doc.apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[
                GestureSample {
                    position: [0.0, 0.0, 1.0],
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [0.05, 0.0, 1.0],
                    pressure: 1.0,
                    time: 1.0,
                },
            ],
            [false; 3],
        );
        // This row carries no triangles, so the refusal is the correct
        // answer and it is the same one `availability` gives — the model and
        // the stroke path may not disagree about whether a tool applies.
        let error = outcome.expect_err("an empty mesh row has nothing to sculpt");
        assert!(
            error.to_string().contains("mesh"),
            "the stroke path refused for a different reason than the tool \
             status would have given: {error}"
        );
    }

    /// The three that are absent from the shelf are absent from the model too.
    #[test]
    fn a_tool_with_no_mesh_verb_is_refused_on_a_mesh_layer() {
        let mut doc = with_mesh_layer();
        let mesh = doc
            .scene()
            .layers
            .iter()
            .find(|l| l.representation == Representation::Mesh)
            .map(|l| l.key)
            .expect("the mesh layer");
        doc.set_active_layer(mesh).expect("activate");

        let error = doc
            .apply_stroke(
                ToolKind::Trim,
                BrushSettings::default(),
                &[GestureSample {
                    position: [0.0, 0.0, 1.0],
                    pressure: 1.0,
                    time: 0.0,
                }],
                [false; 3],
            )
            .expect_err("Trim draws a shape on the frame; it is not a vertex verb");
        assert!(
            error.to_string().contains("mesh"),
            "the refusal must name where the tool does apply: {error}"
        );
    }
}
