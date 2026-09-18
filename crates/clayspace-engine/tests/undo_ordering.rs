//! Which history a step moves, and why a depth could never say.
//!
//! The document keeps records the engine's own history knows nothing about — a
//! mesh gesture, a crossing, the entries a solo left behind — and every one of
//! them has to interleave with the engine's entries in the order the sculptor
//! made them. That ordering used to be a comparison against the engine's undo
//! *depth*: each record remembered the number, and an undo took the record back
//! when the number still matched.
//!
//! A depth is a stack size, and a stack size is not an ordering. Two records
//! made at the same depth both answered "newest", so which one an undo took
//! back was decided by the order the questions happened to be asked in — and a
//! record whose depth was reached again later was matched again, long after the
//! future it described had been built over. Measured before the fix: a mesh
//! stroke taken back, an unrelated edit made, that edit taken back, and the
//! redo meant for the edit was spent re-applying the stroke instead.
//!
//! So the document counts for itself. These hold the counting: one command in,
//! one command out, whichever history it belongs to, and a redo that puts back
//! what the undo before it took.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, Direction, GestureSample, LayerKey, ObjectModel,
    Representation, SceneModel, SculptModel, Shape, ToolKind,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// A document whose active subtool is a carried mesh.
///
/// Through a crossing, which is the route that leaves a `Crossing` record on
/// the undo stack as well — so a test that sculpts here has one of each kind of
/// record to order against the other.
fn mesh_subtool() -> ClayDocument {
    let mut doc = document();
    doc.convert_layer(Direction::SdfToMesh, 0.05, 0)
        .expect("into a mesh");
    doc
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.2,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// One dab at a point, on whatever layer is active.
fn dab(doc: &mut ClayDocument, at: [f32; 3]) {
    doc.apply_stroke(
        ToolKind::Padrao,
        brush(),
        &[GestureSample {
            position: at,
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    )
    .expect("a dab");
}

/// Where the surface stands under a ray straight down -Z at (x, y).
fn height_at(doc: &ClayDocument, x: f32, y: f32) -> Option<f32> {
    SculptModel::pick(doc, [x, y, 4.0], [0.0, 0.0, -1.0]).map(|hit| hit[2])
}

/// One dab that actually lands on the carried mesh under the pointer.
fn dab_on_the_mesh(doc: &mut ClayDocument, x: f32, y: f32) {
    let at = height_at(doc, x, y).expect("the ray met the form");
    dab(doc, [x, y, at]);
}

/// Everything about a document that a history step is supposed to put back.
///
/// The layer set with its names, kinds and eyes; what each layer holds; and the
/// vertices the viewport would draw. The shared harness the per-command history
/// issues were going to need — a command applied and taken back has to leave
/// every one of these where it found them, and a digest says so in one
/// comparison rather than in six assertions per command.
///
/// Which subtool is *active* is deliberately not here. It is a way of looking
/// at the document rather than part of it, the engine's history carries no
/// account of it, and a redone crossing leaves the row it made unselected —
/// which is a gap worth closing and is not this one.
#[derive(Debug, Clone, PartialEq)]
struct Digest {
    layers: Vec<(LayerKey, String, Representation, bool, usize)>,
    carried: Vec<[f32; 3]>,
}

fn digest(doc: &mut ClayDocument) -> Digest {
    let layers = doc
        .scene()
        .layers
        .iter()
        .map(|layer| {
            // A mesh or a grid holds no node list, and "none" is a count like
            // any other for the purpose of comparing two moments.
            let held = doc
                .layer_id(layer.key)
                .ok()
                .and_then(|id| doc.document().layer_nodes(id).ok())
                .map_or(0, |nodes| nodes.len());
            (
                layer.key,
                layer.name.clone(),
                layer.representation,
                layer.visible,
                held,
            )
        })
        .collect();
    Digest {
        layers,
        carried: doc.visible_mesh_geometry().0,
    }
}

/// The layer set alone, which is what an undo must never quietly shorten.
fn layer_keys(doc: &ClayDocument) -> Vec<LayerKey> {
    doc.scene().layers.iter().map(|layer| layer.key).collect()
}

/// Applies one command, takes back everything it cost, and says whether the
/// document came back exactly.
///
/// The harness the sibling history issues are meant to add a line each to.
///
/// What a command costs is read from the history rather than assumed to be
/// one. Most are one; a field dab that made the engine consolidate underneath
/// it is two, which is why the sculpting ViewModel banks a count per action
/// rather than a step. That is a different question from this one — what this
/// asks is whether taking a command back reaches the document it started from,
/// and whether the *entries it took back were its own*.
fn apply_then_undo_restores_exactly(
    doc: &mut ClayDocument,
    what: &str,
    command: impl FnOnce(&mut ClayDocument),
) {
    let before = digest(doc);
    let depth = doc.history().depth;
    command(doc);
    assert_ne!(
        digest(doc),
        before,
        "{what} changed nothing, so taking it back proves nothing"
    );
    let cost = doc.history().depth.saturating_sub(depth);
    assert!(cost >= 1, "{what} left nothing to take back");
    for step in 0..cost {
        assert!(
            doc.undo().expect("undo"),
            "{what} ran out of history at step {step} of {cost}"
        );
    }
    assert_eq!(
        doc.history().depth,
        depth,
        "taking {what} back moved the history somewhere other than where it \
         started"
    );
    assert_eq!(
        digest(doc),
        before,
        "the undo after {what} did not put the document back"
    );
}

// -- one command in, one command out ------------------------------------------

#[test]
fn one_undo_after_a_mesh_stroke_takes_back_the_stroke_and_leaves_the_layers_standing() {
    let mut doc = mesh_subtool();
    let layers = layer_keys(&doc);
    let before = doc.visible_mesh_geometry().0;

    dab_on_the_mesh(&mut doc, 0.0, 0.0);
    let sculpted = doc.visible_mesh_geometry().0;
    assert_ne!(before, sculpted, "the dab moved nothing");

    assert!(doc.undo().expect("undo"), "there was nothing to undo");
    assert_eq!(
        doc.visible_mesh_geometry().0,
        before,
        "the undo did not reach the stroke"
    );
    assert_eq!(
        layer_keys(&doc),
        layers,
        "the undo meant for a stroke took a layer away — which is what an \
         ordering decided by a shared depth does as soon as two records answer \
         to the same number"
    );
}

/// A command taken back leaves the document it started from, whichever history
/// holds it.
#[test]
fn a_command_taken_back_leaves_the_document_it_started_from() {
    let mut doc = document();
    apply_then_undo_restores_exactly(&mut doc, "a dab on the field", |doc| {
        dab(doc, [0.0, 0.0, 1.0]);
    });
    apply_then_undo_restores_exactly(&mut doc, "a second subtool", |doc| {
        doc.add_layer("Segunda", Representation::Sdf)
            .expect("another subtool");
    });
    apply_then_undo_restores_exactly(&mut doc, "a placed shape", |doc| {
        doc.place_object(
            Shape::Sphere,
            &[0.4],
            [0.0; 3],
            CombineSettings {
                op: Combine::Add,
                ..CombineSettings::default()
            },
        )
        .expect("a form");
    });
    apply_then_undo_restores_exactly(&mut doc, "a crossing to a mesh", |doc| {
        doc.convert_layer(Direction::SdfToMesh, 0.05, 0)
            .expect("into a mesh");
    });
    // The pair the ordering exists for: a crossing, which is an engine entry
    // the document names, and a gesture over the mesh it made, which is not an
    // engine entry at all. Taken back together here, so the run reaches the
    // field it started from.
    apply_then_undo_restores_exactly(&mut doc, "a crossing and a stroke on it", |doc| {
        doc.convert_layer(Direction::SdfToMesh, 0.05, 0)
            .expect("into a mesh");
        dab_on_the_mesh(doc, 0.0, 0.0);
    });
}

/// A depth that saturates, coalesces or is simply reached twice cannot say
/// which of two records is newer. A stamp can, and this is the shape that used
/// to prove it could not.
#[test]
fn a_run_of_commands_undoes_one_at_a_time_in_the_order_it_was_made() {
    let mut doc = mesh_subtool();
    let mut moments = vec![digest(&mut doc)];
    for step in 0..6 {
        dab_on_the_mesh(&mut doc, -0.2 + 0.08 * step as f32, 0.0);
        moments.push(digest(&mut doc));
    }
    doc.add_layer("Depois", Representation::Sdf)
        .expect("one more subtool");
    moments.push(digest(&mut doc));

    for expected in moments.iter().rev().skip(1) {
        assert!(
            doc.undo().expect("undo"),
            "the run ran out of history early"
        );
        assert_eq!(
            digest(&mut doc),
            *expected,
            "an undo landed somewhere other than the moment before the command \
             it was meant for"
        );
    }
}

// -- what a redo is for -------------------------------------------------------

/// A gesture taken back before a new edit is not put back after it.
///
/// The defect this file exists for, at its sharpest. A mesh gesture costs the
/// engine no entry, so the engine's own truncation of its redo stack never
/// reached the record — it sat on this side remembering a depth, and the next
/// undo that happened to bring the depth back to that number handed it to a
/// redo the sculptor meant for something else. Measured before the fix: the
/// stroke came back and the subtool did not.
#[test]
fn a_mesh_gesture_undone_before_a_new_edit_is_not_put_back_after_it() {
    let mut doc = mesh_subtool();
    let before = doc.visible_mesh_geometry().0;

    dab_on_the_mesh(&mut doc, 0.0, 0.0);
    assert_ne!(
        doc.visible_mesh_geometry().0,
        before,
        "the dab moved nothing"
    );
    assert!(doc.undo().expect("undo"), "the stroke was not undoable");
    assert_eq!(doc.visible_mesh_geometry().0, before);

    // A new edit, which makes the undone stroke unreachable exactly as it makes
    // the engine's own redo stack unreachable.
    doc.add_layer("Depois", Representation::Sdf)
        .expect("another subtool");
    let with_the_layer = layer_keys(&doc);

    assert!(doc.undo().expect("undo"), "the subtool was not undoable");
    assert_eq!(
        layer_keys(&doc).len(),
        with_the_layer.len() - 1,
        "the undo did not take the subtool away"
    );

    assert!(doc.redo().expect("redo"), "the subtool did not come back");
    assert_eq!(
        layer_keys(&doc),
        with_the_layer,
        "the redo was spent on the stranded stroke instead of the subtool it \
         was meant for"
    );
    assert_eq!(
        doc.visible_mesh_geometry().0,
        before,
        "a stroke the sculptor had already taken back was put on again by a \
         redo meant for a later command"
    );
}

/// Apply, take it all back, put it all forward: the document is where it was.
#[test]
fn redo_restores_what_undo_took_back() {
    let mut doc = mesh_subtool();
    let start = digest(&mut doc);

    // One of each kind, in an order that puts a record from every stack under
    // one from another: a gesture the engine never sees, an engine entry over
    // it, an engine entry that coalesces, and a crossing.
    let depth = doc.history().depth;
    dab_on_the_mesh(&mut doc, -0.1, 0.0);
    doc.add_layer("Campo", Representation::Sdf)
        .expect("a field subtool");
    dab(&mut doc, [0.0, 0.0, 1.0]);
    doc.convert_layer(Direction::SdfToVoxel, 0.05, 0)
        .expect("into a grid");
    let finished = digest(&mut doc);
    assert_ne!(finished, start, "the run changed nothing");
    // What the run cost, read from the history rather than counted in
    // commands: a field dab that made the engine consolidate under it is two
    // entries, and undoing four of five would leave the run half taken back.
    let made = doc.history().depth.saturating_sub(depth);

    for step in 0..made {
        assert!(
            doc.undo().expect("undo"),
            "step {step} back ran out of history"
        );
    }
    assert_eq!(
        digest(&mut doc),
        start,
        "undoing every command did not reach the document the run started from"
    );

    for step in 0..made {
        assert!(
            doc.redo().expect("redo"),
            "step {step} forward ran out of history"
        );
    }
    assert_eq!(
        digest(&mut doc),
        finished,
        "redoing every command did not reach the document the run ended at"
    );
}

// -- what the interface is told ----------------------------------------------

/// The depth rises for every command that changes the document.
///
/// Including the ones the engine records nothing for. A history that counted
/// only the engine's entries greyed out Undo in the middle of a mesh sculpting
/// session; one that counted a solo's entries said a fresh document had three
/// things to take back.
#[test]
fn every_command_that_changes_the_document_is_one_step_of_history() {
    let mut doc = mesh_subtool();
    let start = doc.history().depth;

    dab_on_the_mesh(&mut doc, -0.1, 0.0);
    assert_eq!(
        doc.history().depth,
        start + 1,
        "a mesh stroke was not a step"
    );

    doc.add_layer("Campo", Representation::Sdf)
        .expect("a field subtool");
    assert_eq!(
        doc.history().depth,
        start + 2,
        "a new subtool was not a step"
    );

    dab(&mut doc, [0.0, 0.0, 1.0]);
    assert_eq!(doc.history().depth, start + 3, "a field dab was not a step");

    doc.convert_layer(Direction::SdfToVoxel, 0.05, 0)
        .expect("into a grid");
    assert_eq!(
        doc.history().depth,
        start + 4,
        "a crossing counted as more than the one thing the sculptor asked for"
    );

    let soloed = doc.scene().active.expect("an active subtool");
    doc.set_solo(Some(soloed)).expect("show it alone");
    assert_eq!(
        doc.history().depth,
        start + 4,
        "a way of looking at the scene was counted as something to take back"
    );
}
