//! Recorded passes on a voxel grid: dialling one back, and what a stack costs.
//!
//! A pass a sculptor can dial back after making it — ZBrush's layers, on a
//! grid. Not undo, which is a stack you pop; this is a slider you keep. What it
//! stores is what the pass *changed* rather than the brushes that changed it,
//! so dialling one replays cells and does not re-run strokes.
//!
//! Measured through the surface rather than through the stack's own numbers
//! wherever it can be: a strength that is stored and never replayed reads
//! exactly like one that works.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, GestureSample, Representation, SceneModel, SculptLayerOp,
    SculptModel, ToolKind,
};

/// A document whose active layer is a grid, made by crossing the starting form
/// over and dropping the field it came from — so the surface measured below is
/// the grid's and nothing else's.
fn with_grid() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let source = document.scene().active.expect("a starting layer");
    document
        .convert_layer(Direction::SdfToVoxel, 0.04, 1)
        .expect("cross to a grid");
    document.remove_layer(source).expect("drop the field");
    document
}

fn stroke(document: &mut ClayDocument) -> bool {
    let brush = BrushSettings {
        size: 0.3,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    let samples: Vec<GestureSample> = (0..6)
        .map(|i| {
            let t = i as f32 / 5.0;
            GestureSample {
                position: [(t - 0.5) * 0.5, 0.0, 1.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])
        .map(|outcome| outcome.changed)
        .unwrap_or(false)
}

/// The passes on the active layer.
fn passes(document: &ClayDocument) -> Vec<clayspace_model::SculptLayer> {
    let scene = document.scene();
    scene
        .active
        .and_then(|key| scene.layer(key).cloned())
        .map(|layer| layer.sculpt_layers)
        .unwrap_or_default()
}

/// How many cells the grid holds.
///
/// The grid itself rather than anything derived from it. A raycast would work —
/// a voxel layer *is* evaluated into the document's field, and `pick` finds it
/// — but it answers with whichever surface is nearest, and a pass dialled
/// between 0 and 1 moves the surface by a fraction of a cell. Counting cells
/// measures the pass in the unit the pass records in, and cannot be fooled by
/// a fraction landing between two rays.
fn cells(document: &mut ClayDocument) -> usize {
    document
        .occupied_cells()
        .expect("the active layer is a grid")
}

#[test]
fn a_recorded_pass_appears_in_the_stack_with_what_it_changed() {
    let mut document = with_grid();
    assert!(
        passes(&document).is_empty(),
        "a fresh grid carries no passes"
    );

    document
        .apply_sculpt_layer_op(SculptLayerOp::BeginRecording {
            name: "Detalhe".into(),
        })
        .expect("begin recording");
    assert!(
        document.sculpt_layer_cost().recording,
        "the shell has no way to tell a sculptor a pass is being recorded"
    );
    assert!(stroke(&mut document), "the stroke reached no cell");
    document
        .apply_sculpt_layer_op(SculptLayerOp::EndRecording)
        .expect("end recording");
    assert!(!document.sculpt_layer_cost().recording);

    let stack = passes(&document);
    assert_eq!(stack.len(), 1, "the pass was not recorded");
    assert_eq!(stack[0].name, "Detalhe");
    assert!(
        !stack[0].is_empty(),
        "the pass recorded no cells, so there is nothing to dial"
    );
    assert!(
        stack[0].bytes > 0,
        "the pass reports no cost, so the panel cannot say what the stack occupies"
    );

    let cost = document.sculpt_layer_cost();
    assert_eq!(cost.layers, 1);
    assert_eq!(cost.bytes, stack[0].bytes, "the total is not the sum");
}

/// The whole point: the strength stays adjustable after the strokes are
/// finished, and dialling it to zero gives back the grid without the pass.
#[test]
fn dialling_a_pass_to_zero_gives_back_the_grid_without_it() {
    let mut document = with_grid();
    let before = cells(&mut document);

    document
        .apply_sculpt_layer_op(SculptLayerOp::BeginRecording {
            name: String::new(),
        })
        .expect("begin");
    assert!(stroke(&mut document));
    document
        .apply_sculpt_layer_op(SculptLayerOp::EndRecording)
        .expect("end");
    let applied = cells(&mut document);
    assert_ne!(before, applied, "the pass changed nothing to dial");

    document
        .apply_sculpt_layer_op(SculptLayerOp::SetStrength {
            index: 0,
            strength: 0.0,
        })
        .expect("dial it away");
    assert_eq!(
        cells(&mut document),
        before,
        "strength 0 is exact — it is the grid without the pass — and this grid \
         is not it"
    );

    // And back. Strength 1 is exact too: the pass applied directly.
    document
        .apply_sculpt_layer_op(SculptLayerOp::SetStrength {
            index: 0,
            strength: 1.0,
        })
        .expect("dial it back");
    assert_eq!(cells(&mut document), applied);
}

/// A pass is not undo, and this is the difference that matters: the strength
/// is a property of the stack, so changing it is not something undo takes back
/// and the strokes are still there at whatever it is set to.
#[test]
fn the_strength_is_read_back_from_the_stack() {
    let mut document = with_grid();
    document
        .apply_sculpt_layer_op(SculptLayerOp::BeginRecording {
            name: String::new(),
        })
        .expect("begin");
    assert!(stroke(&mut document));
    document
        .apply_sculpt_layer_op(SculptLayerOp::EndRecording)
        .expect("end");

    document
        .apply_sculpt_layer_op(SculptLayerOp::SetStrength {
            index: 0,
            strength: 0.4,
        })
        .expect("dial");
    assert!(
        (passes(&document)[0].strength - 0.4).abs() < 1e-4,
        "the stack reports {} after being set to 0.4",
        passes(&document)[0].strength
    );
}

#[test]
fn a_pass_can_be_hidden_and_shown() {
    let mut document = with_grid();
    let before = cells(&mut document);
    document
        .apply_sculpt_layer_op(SculptLayerOp::BeginRecording {
            name: String::new(),
        })
        .expect("begin");
    assert!(stroke(&mut document));
    document
        .apply_sculpt_layer_op(SculptLayerOp::EndRecording)
        .expect("end");

    document
        .apply_sculpt_layer_op(SculptLayerOp::SetVisible {
            index: 0,
            visible: false,
        })
        .expect("hide");
    assert!(!passes(&document)[0].visible);
    assert_eq!(
        cells(&mut document),
        before,
        "a hidden pass is still showing"
    );
}

#[test]
fn a_pass_can_be_removed() {
    let mut document = with_grid();
    document
        .apply_sculpt_layer_op(SculptLayerOp::BeginRecording {
            name: String::new(),
        })
        .expect("begin");
    assert!(stroke(&mut document));
    document
        .apply_sculpt_layer_op(SculptLayerOp::EndRecording)
        .expect("end");
    assert_eq!(passes(&document).len(), 1);

    document
        .apply_sculpt_layer_op(SculptLayerOp::Remove { index: 0 })
        .expect("remove");
    assert!(passes(&document).is_empty());
}

/// Two passes, recorded one after the other, so ordering and merging have
/// something to act on.
fn with_two_passes() -> ClayDocument {
    let mut document = with_grid();
    for (index, name) in ["Base", "Detalhe"].into_iter().enumerate() {
        document
            .apply_sculpt_layer_op(SculptLayerOp::BeginRecording { name: name.into() })
            .expect("begin recording");
        // Offset, so the two passes touch different cells and merging them
        // means something.
        let brush = BrushSettings {
            size: 0.3,
            intensity: 1.0,
            ..BrushSettings::default()
        };
        let samples: Vec<GestureSample> = (0..6)
            .map(|i| {
                let t = i as f32 / 5.0;
                GestureSample {
                    position: [(t - 0.5) * 0.5, index as f32 * 0.3, 1.0],
                    pressure: 1.0,
                    time: t,
                }
            })
            .collect();
        document
            .apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])
            .expect("sculpt");
        document
            .apply_sculpt_layer_op(SculptLayerOp::EndRecording)
            .expect("end recording");
    }
    document
}

/// Merging folds a pass into the one below and keeps the lower layer's name,
/// which is the whole point of the verb: one entry per cell instead of two.
#[test]
fn merging_a_pass_down_leaves_one_pass_with_the_lower_name() {
    let mut document = with_two_passes();
    assert_eq!(passes(&document).len(), 2, "the fixture recorded one pass");
    let before = cells(&mut document);

    document
        .apply_sculpt_layer_op(SculptLayerOp::MergeDown { index: 1 })
        .expect("merge the upper pass down");

    let stack = passes(&document);
    assert_eq!(stack.len(), 1, "merging did not fold the two into one");
    assert_eq!(
        stack[0].name, "Base",
        "merging kept the upper pass's name; the engine's contract is that the \
         lower one survives"
    );
    assert_eq!(
        cells(&mut document),
        before,
        "merging at full strength changed the surface, and it is a bookkeeping \
         verb rather than an edit"
    );
}

/// The bottom pass has nothing below it, and the refusal is the engine's.
#[test]
fn the_bottom_pass_cannot_be_merged_down() {
    let mut document = with_two_passes();
    assert!(document
        .apply_sculpt_layer_op(SculptLayerOp::MergeDown { index: 0 })
        .is_err());
}

/// Order is meaningful — where two passes touched the same cell, moving one
/// past the other changes which value survives — so the stack has to actually
/// reorder rather than relabel.
#[test]
fn moving_a_pass_changes_the_order_of_the_stack() {
    let mut document = with_two_passes();
    let names: Vec<String> = passes(&document).iter().map(|p| p.name.clone()).collect();
    assert_eq!(names, vec!["Base", "Detalhe"]);

    document
        .apply_sculpt_layer_op(SculptLayerOp::Move { from: 1, to: 0 })
        .expect("move the upper pass to the bottom");

    let names: Vec<String> = passes(&document).iter().map(|p| p.name.clone()).collect();
    assert_eq!(names, vec!["Detalhe", "Base"], "the stack did not reorder");
}

/// Passes belong to a grid, and the refusal has to say so rather than failing
/// generically on a layer that has no stack to record into.
#[test]
fn a_pass_on_a_field_is_refused_by_where_it_applies() {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    assert_eq!(document.active_representation(), Representation::Sdf);
    let error = document
        .apply_sculpt_layer_op(SculptLayerOp::BeginRecording {
            name: String::new(),
        })
        .expect_err("a field has no stack to record into");
    assert!(
        error.to_string().contains("voxel"),
        "the refusal must name where a pass applies: {error}"
    );
}
