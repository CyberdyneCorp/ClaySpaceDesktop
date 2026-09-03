//! The normals a mesh gesture defers, and the flush that owes them.
//!
//! `stroke_mesh` holds a segment's normals back so the overlapping dabs of one
//! stroke are recomputed once instead of once each. That trade is only ever a
//! trade about *when*: the committed form has to shade from where its vertices
//! are, and the gesture's undo has to put the shading back as well as the
//! vertices. Both are silent when they are wrong — a form shaded from where it
//! used to be reads as a lighting bug, and an undo that restores post-stroke
//! normals reads as nothing at all until the form is turned to the light.
//!
//! So the risk is not the commit. It is every *other* way a stroke ends, and
//! there is one of these per exit: committed, cancelled, the tool changed
//! under it, the subtool changed under it, an undo taken mid-drag, and the
//! document going away while a gesture is still open. What makes them all hold
//! is that the record and the sculptor are one value — `LiveMesh` — whose
//! `Drop` settles, rather than a call written at the end of each path.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, GestureSample, LatticeModel, ObjectModel,
    Representation, SceneModel, SculptModel, Shape, ToolKind,
};

fn mesh_form() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    document
        .convert_layer(clayspace_model::Direction::SdfToMesh, 0.02, 0)
        .expect("into a mesh");
    document
}

/// The mesh subtools, in stack order.
fn mesh_layers(document: &ClayDocument) -> Vec<clayspace_model::LayerKey> {
    document
        .scene()
        .layers
        .iter()
        .filter(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .collect()
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.25,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// Positions and normals as the viewport would read them.
#[derive(Clone, PartialEq)]
struct Form {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
}

fn form(document: &mut ClayDocument) -> Form {
    let (positions, normals, _, _, _) = document.visible_mesh_geometry();
    assert!(!positions.is_empty(), "the fixture carries no triangles");
    assert_eq!(normals.len(), positions.len(), "a normal per vertex");
    Form { positions, normals }
}

/// How many vertices moved, and how many of those are still shaded from where
/// they were.
///
/// Bit-exact rather than within a tolerance, and that is what gives it teeth:
/// a flush that never happened leaves *every* moved vertex's normal byte for
/// byte what it was, so the second figure is the first. A flush that happened
/// leaves almost none — a vertex would have to move in a way that recomputes
/// to the same three floats.
fn moved_and_stale(before: &Form, after: &Form) -> (usize, usize) {
    let mut moved = 0;
    let mut stale = 0;
    for i in 0..before.positions.len() {
        if before.positions[i] != after.positions[i] {
            moved += 1;
            if before.normals[i] == after.normals[i] {
                stale += 1;
            }
        }
    }
    (moved, stale)
}

/// The form shades from where its vertices are.
///
/// `where_` names the exit, because that is the whole subject of this file.
fn assert_settled(before: &Form, after: &Form, where_: &str) {
    let (moved, stale) = moved_and_stale(before, after);
    assert!(
        moved > 0,
        "{where_}: the gesture moved no vertex at all, so this case is not \
         testing a deferral"
    );
    assert!(
        stale * 4 < moved,
        "{where_}: {stale} of {moved} moved vertices still carry the normal \
         they had before the gesture. A flush that never happened leaves all \
         of them"
    );
}

/// The gesture every case here draws: three overlapping segments down the
/// front of the form, delivered the way the ViewModel delivers a stamping
/// tool — only what the model has not seen.
const SEGMENTS: [[f32; 3]; 3] = [[0.0, 0.0, 1.0], [0.12, 0.06, 0.98], [0.24, 0.12, 0.92]];

/// Where the gesture takes hold, for the dragging verb.
const ANCHOR: [f32; 3] = [0.0, 0.0, 1.0];

fn segment(document: &mut ClayDocument, tool: ToolKind, at: [f32; 3]) {
    document
        .apply_stroke(
            tool,
            brush(),
            &[
                GestureSample {
                    position: at,
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [at[0] + 0.04, at[1], at[2]],
                    pressure: 1.0,
                    time: 1.0,
                },
            ],
            [false; 3],
        )
        .expect("the mesh refused a dab");
}

/// One segment of a Grab drag, replayed from the anchor the way the ViewModel
/// delivers a dragging verb.
///
/// **Grab is the verb these exits are written on, and that is not arbitrary.**
/// Fifteen of the sixteen mesh verbs go through the engine's own stroke
/// resolver, which carries its own deferral and settles at the end of the
/// stroke it drove, because there the library knows where the stroke ended.
/// Grab is the one that does not: it is a single stamp per mirror, it reads
/// the sculptor's own flag, and the flush is the host's to make. So a case
/// written on Padrão would pass whether or not this application ever flushed
/// anything.
fn drag(document: &mut ClayDocument, by: f32) {
    document
        .apply_stroke(
            ToolKind::Mover,
            brush(),
            &[
                GestureSample {
                    position: ANCHOR,
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [ANCHOR[0] + by, ANCHOR[1], ANCHOR[2]],
                    pressure: 1.0,
                    time: 1.0,
                },
            ],
            [false; 3],
        )
        .expect("the mesh refused a drag");
}

/// The plain exit, and the floor the others stand on.
#[test]
fn a_committed_gesture_shades_from_where_its_vertices_are() {
    let mut document = mesh_form();
    let before = form(&mut document);

    document.begin_gesture();
    for by in [0.1, 0.2, 0.3] {
        drag(&mut document, by);
    }
    document.end_gesture();

    assert_settled(&before, &form(&mut document), "committed");
}

/// The other half of the pair: a resolved stroke's own deferral, which is the
/// library's to settle rather than this application's. Held here so that the
/// two switches are both exercised and the difference between them is on the
/// record.
#[test]
fn a_resolved_stroke_shades_from_where_its_vertices_are_too() {
    let mut document = mesh_form();
    let before = form(&mut document);

    document.begin_gesture();
    for at in SEGMENTS {
        segment(&mut document, ToolKind::Padrao, at);
    }
    document.end_gesture();

    assert_settled(&before, &form(&mut document), "a resolved stroke");
}

/// And its record is exact, which is the half a flush into the wrong record
/// would lose: the vertices come back and the shading does not.
#[test]
fn a_committed_gesture_undoes_bit_exactly_including_its_shading() {
    let mut document = mesh_form();
    let before = form(&mut document);

    document.begin_gesture();
    for by in [0.1, 0.2, 0.3] {
        drag(&mut document, by);
    }
    document.end_gesture();
    assert_ne!(
        form(&mut document).positions,
        before.positions,
        "the gesture reached nothing, so the undo below proves nothing"
    );

    assert!(
        SculptModel::undo(&mut document).expect("undo"),
        "nothing to undo"
    );
    let back = form(&mut document);
    assert_eq!(
        back.positions, before.positions,
        "the record did not put the vertices back"
    );
    assert_eq!(
        back.normals, before.normals,
        "the record put the vertices back and left the shading where the \
         gesture wrote it — which is what flushing into a record other than \
         the one the stamps were noted into looks like"
    );

    assert!(
        SculptModel::redo(&mut document).expect("redo"),
        "nothing to redo"
    );
    assert_settled(&before, &form(&mut document), "redone");
}

/// Cancel is the exit that reads the record hardest: the ViewModel ends the
/// gesture and then spends the history taking it back, so a record missing its
/// normals leaves a form that was never touched shaded as though it had been.
#[test]
fn a_cancelled_gesture_leaves_the_form_exactly_as_it_found_it() {
    let mut document = mesh_form();
    let before = form(&mut document);

    document.begin_gesture();
    for by in [0.1, 0.2, 0.3] {
        drag(&mut document, by);
    }
    // What `SculptViewModel`'s CancelStroke does: close the gesture so the
    // preview is banked, then undo what the gesture spent.
    document.end_gesture();
    // One step, not the whole history: a gesture is one undo however many
    // segments drew it, and taking more back would undo the crossing that made
    // the mesh.
    assert!(
        SculptModel::undo(&mut document).expect("undo"),
        "nothing to cancel"
    );

    let back = form(&mut document);
    assert_eq!(
        back.positions, before.positions,
        "cancelling left the vertices somewhere else"
    );
    assert_eq!(
        back.normals, before.normals,
        "cancelling put the vertices back and left the shading behind"
    );
}

/// A tool changed under an open gesture, which is what a shortcut pressed
/// mid-drag does. The record continues or is replaced depending on whether the
/// incoming verb drags or stamps, and either way the segment that came before
/// has already settled.
#[test]
fn a_tool_changed_mid_drag_settles_what_the_last_one_deferred() {
    let mut document = mesh_form();
    let before = form(&mut document);

    document.begin_gesture();
    // Padrão and Inflar stamp, Mover drags — so the drag retires the stamping
    // record rather than continuing it, which is the boundary the
    // settle-before-revert ordering exists for.
    segment(&mut document, ToolKind::Padrao, SEGMENTS[0]);
    let after_first = form(&mut document);
    assert_settled(&before, &after_first, "the first tool's segment");

    segment(&mut document, ToolKind::Inflar, SEGMENTS[1]);
    // Ending on the dragging verb, so what is owed at the end of the gesture
    // is owed to this application rather than to the stroke resolver.
    drag(&mut document, 0.25);
    document.end_gesture();

    assert_settled(&before, &form(&mut document), "after the tools changed");
    assert!(
        SculptModel::undo(&mut document).expect("undo"),
        "nothing to undo"
    );
    let back = form(&mut document);
    assert_eq!(back.positions, before.positions);
    assert_eq!(
        back.normals, before.normals,
        "a gesture that changed tools left shading behind that no record put \
         back"
    );
}

/// A subtool changed under an open gesture. The gesture on the old layer is
/// dropped rather than carried — and dropping one settles it, which is the
/// exit no call at the end of a path would have covered.
#[test]
fn a_subtool_changed_mid_drag_settles_the_gesture_it_abandons() {
    let mut document = mesh_form();
    // A crossing leaves the field layer standing and adds the mesh beside it,
    // so the mesh subtools are the ones to name here.
    let first = mesh_layers(&document)[0];
    SceneModel::add_layer(&mut document, "Segunda", Representation::Sdf).expect("a second subtool");
    document
        .place_object(
            Shape::Sphere,
            &[0.6],
            [0.0; 3],
            CombineSettings {
                op: Combine::Add,
                ..CombineSettings::default()
            },
        )
        .expect("a form in the second subtool");
    document
        .convert_layer(clayspace_model::Direction::SdfToMesh, 0.02, 0)
        .expect("the second into a mesh");
    let second = mesh_layers(&document)[1];

    SceneModel::set_active_layer(&mut document, first).expect("the first subtool");
    let before = form(&mut document);

    document.begin_gesture();
    drag(&mut document, 0.25);
    // The pointer lands on the other subtool without the gesture ever ending,
    // which is what the shelf's own switch does mid-drag. The gesture held on
    // the first subtool is dropped rather than carried — and dropping it is
    // the only thing that settles it.
    SceneModel::set_active_layer(&mut document, second).expect("the second subtool");
    drag(&mut document, 0.25);
    document.end_gesture();

    // Both meshes are read together, so the figure covers the subtool the
    // gesture abandoned as well as the one it moved on to: a flush skipped on
    // the abandoned one leaves half the moved vertices stale, which is well
    // past what this allows.
    assert_settled(&before, &form(&mut document), "the abandoned subtool");
}

/// An undo taken while the pointer is still down. Nothing is owed between
/// segments — the deferral never outlives the call that armed it — so the
/// revert writes over a mesh whose shading already agrees with it.
#[test]
fn an_undo_taken_mid_drag_finds_the_shading_already_settled() {
    let mut document = mesh_form();

    // One committed stroke for the undo to have something to take back.
    document.begin_gesture();
    segment(&mut document, ToolKind::Padrao, SEGMENTS[2]);
    document.end_gesture();
    let after_first = form(&mut document);

    document.begin_gesture();
    drag(&mut document, 0.2);
    let mid_drag = form(&mut document);
    assert_settled(&after_first, &mid_drag, "mid-drag");

    // Cmd+Z with the pointer still down.
    assert!(
        SculptModel::undo(&mut document).expect("undo"),
        "nothing to undo"
    );
    drag(&mut document, 0.3);
    document.end_gesture();

    assert_settled(&after_first, &form(&mut document), "after an undo mid-drag");
}

/// The exit that is not a code path at all: the document goes away while a
/// gesture is still open.
///
/// This is a regression test with a measured failure behind it. `LiveMesh`
/// settles on `Drop`, and settling reads the layer's mesh — so with the
/// gesture declared *after* the document in `ClayDocument`, the meshes were
/// freed first and the flush read storage that had gone. A segmentation fault
/// inside the engine, not a refusal, because a borrowed handle has nothing
/// left to check against.
#[test]
fn a_document_dropped_mid_drag_settles_before_its_meshes_go() {
    let mut document = mesh_form();
    document.begin_gesture();
    for by in [0.1, 0.2, 0.3] {
        drag(&mut document, by);
    }
    // No `end_gesture`: the gesture is open, the record is held, and the whole
    // document goes.
    drop(document);
}

/// The same, through the cage's own gesture lifecycle, which never touches
/// `begin_gesture` or `end_gesture` at all.
#[test]
fn a_document_dropped_under_a_cage_preview_settles_the_same_way() {
    let mut document = mesh_form();
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let points = document.lattice().points.clone();
    document.select_lattice_point(Some(0));
    document
        .drag_lattice_point([points[0][0], points[0][1] + 0.3, points[0][2]])
        .expect("the cage was refused");
    drop(document);
}
