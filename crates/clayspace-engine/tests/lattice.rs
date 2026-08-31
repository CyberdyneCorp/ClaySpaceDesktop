//! The cage around the form, and what dragging its points does.
//!
//! ZBrush spells it the Gizmo Lattice, Blender the Lattice modifier, Maya an
//! FFD. All three do the same thing and this is the engine half of it: a box
//! of control points around the model, dragged, with the form following.
//!
//! There are two routes underneath and the difference is not cosmetic. A mesh
//! is deformed *forward* — each vertex evaluated once, exact, up to 32 points
//! per axis. A field is deformed by an inverse point map resolved into one
//! deformer per item and evaluated at every sample, which is why the engine
//! caps that one at 4. A grid has neither.

use clayspace_engine::{BackendPolicy, ClayDocument};
/// A camera in front of the form, for the drags that do not use the outer
/// ring — every handle carries the direction the camera faced, and only that
/// one reads it.
const LOOKING_DOWN_Z: [f32; 3] = [0.0, 0.0, 1.0];

use clayspace_model::{
    Direction, GizmoHandle, GizmoMode, LatticeModel, Representation, SculptModel,
};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

fn meshed() -> ClayDocument {
    let mut document = sphere();
    document
        .convert_layer(Direction::SdfToMesh, 0.03, 0)
        .expect("cross to a mesh");
    document
}

/// The highest point of the drawn mesh.
fn top(document: &mut ClayDocument) -> f32 {
    document
        .visible_mesh_geometry()
        .0
        .iter()
        .map(|v| v[1])
        .fold(f32::MIN, f32::max)
}

/// How far the field reaches along +z.
fn reach(document: &ClayDocument) -> f32 {
    SculptModel::pick(document, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .map(|hit| hit[2])
        .expect("the near face")
}

/// Drags every control point above the middle by `by`, on `axis`.
fn lift(document: &mut ClayDocument, axis: usize, by: f32) {
    let cage = document.lattice();
    for (index, point) in cage.points.iter().enumerate() {
        if point[axis] <= 0.0 {
            continue;
        }
        document.select_lattice_point(Some(index));
        let mut to = *point;
        to[axis] += by;
        document
            .drag_lattice_point(to)
            .expect("the drag was refused");
    }
}

#[test]
fn a_cage_wraps_the_form_rather_than_a_fixed_box() {
    // Sized to what the layer contains. A cage that does not enclose the form
    // has control points with nothing under them, and the corners a sculptor
    // reaches for first would be the ones that do least.
    let mut document = meshed();
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let cage = document.lattice();

    assert!(cage.active);
    assert_eq!(cage.points.len(), 8, "a 2x2x2 cage has eight corners");
    assert_eq!(cage.edges().len(), 12, "a cube has twelve edges");
    assert!(!cage.touched, "a fresh cage is already dragged");

    // Around the unit sphere, and a little proud of it: a corner point buried
    // in the clay is not a handle.
    let (min, max) = (cage.points[0], cage.points[7]);
    for axis in 0..3 {
        assert!(
            min[axis] < -1.0 && max[axis] > 1.0,
            "the cage spans {}..{} on axis {axis}, which does not enclose a \
             unit sphere with room to grab it",
            min[axis],
            max[axis]
        );
        assert!(
            min[axis] > -1.3 && max[axis] < 1.3,
            "the cage stands {}..{} off a unit sphere, which is a box in the \
             distance rather than a cage around the form",
            min[axis],
            max[axis]
        );
    }
}

#[test]
fn dragging_the_cage_bends_a_mesh() {
    let mut document = meshed();
    let before = top(&mut document);
    document.begin_lattice([2, 2, 2]).expect("a cage");
    lift(&mut document, 1, 0.5);

    assert!(document.lattice().touched, "the drags did not register");
    // Nothing has moved yet: a cage is worked in, and the form follows when it
    // is applied.
    document.apply_lattice().expect("the cage was refused");

    let after = top(&mut document);
    assert!(
        (after - (before + 0.5)).abs() < 0.05,
        "the top went from {before} to {after}, where the four corners above \
         it were pulled up by 0.5 — a corner control point is interpolated, so \
         dragging one moves that corner of the box exactly"
    );
    assert!(
        !document.lattice().active,
        "the cage stayed up after being applied"
    );
}

#[test]
fn dragging_the_cage_bends_a_field() {
    // The other route: an inverse point map resolved into one deformer per
    // item, which is what lets a field be caged at all.
    let mut document = sphere();
    let before = reach(&document);
    document.begin_lattice([4, 4, 4]).expect("a cage");
    lift(&mut document, 2, 0.5);
    document.apply_lattice().expect("the cage was refused");

    let after = reach(&document);
    assert!(
        after > before + 0.3,
        "the field reached {after} from {before} after its near face was \
         pulled forward by 0.5"
    );
}

#[test]
fn a_cage_is_one_undo_however_many_points_were_dragged() {
    // The unit a sculptor thinks in: they bent the form once.
    let mut document = meshed();
    let before = document.history().depth;
    let shape = top(&mut document);
    document.begin_lattice([3, 3, 3]).expect("a cage");
    lift(&mut document, 1, 0.4);
    document.apply_lattice().expect("the cage was refused");

    assert_eq!(
        document.history().depth,
        before + 1,
        "bending through a cage left more than one entry on the stack, so \
         undoing it takes as many presses as points were dragged"
    );
    document.undo().expect("undo");
    assert!(
        (top(&mut document) - shape).abs() < 1e-3,
        "one undo did not give the form back"
    );
}

#[test]
fn an_untouched_cage_changes_nothing() {
    // An untouched cage is exactly the identity, and applying one pays for a
    // pass over every vertex to move them all by zero.
    let mut document = meshed();
    let before = document.history().depth;
    let shape = top(&mut document);
    document.begin_lattice([3, 3, 3]).expect("a cage");
    document.apply_lattice().expect("the cage was refused");

    assert_eq!(document.history().depth, before, "it recorded an edit");
    assert!(
        (top(&mut document) - shape).abs() < 1e-6,
        "it moved the form"
    );
}

#[test]
fn cancelling_a_cage_leaves_the_form_alone() {
    let mut document = meshed();
    let shape = top(&mut document);
    document.begin_lattice([2, 2, 2]).expect("a cage");
    lift(&mut document, 1, 0.5);
    document.cancel_lattice();

    assert!(!document.lattice().active);
    assert!(
        (top(&mut document) - shape).abs() < 1e-6,
        "abandoning a cage bent the form anyway"
    );
}

#[test]
fn a_grid_takes_no_cage_and_says_so() {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document.add_voxel_layer("Voxels", 0.05).expect("a grid");

    let refused = document
        .begin_lattice([2, 2, 2])
        .expect_err("caging a grid");
    let said = refused.to_string();
    assert!(
        said.contains("SDF") || said.contains("malha"),
        "the refusal does not name the way round: {said}"
    );
    assert!(!document.lattice().active);
}

#[test]
fn the_cage_is_as_fine_as_the_representation_allows() {
    // Not the same ceiling on both, and the difference is the mechanism: a
    // mesh is evaluated once per vertex, a field at every sample.
    let mut mesh = meshed();
    mesh.begin_lattice([32, 32, 32]).expect("a cage");
    assert_eq!(mesh.lattice().divisions, [32; 3]);
    assert_eq!(mesh.lattice().points.len(), 32 * 32 * 32);

    let mut field = sphere();
    field.begin_lattice([32, 32, 32]).expect("a cage");
    assert_eq!(
        field.lattice().divisions,
        [4; 3],
        "a field cage went past the four points per axis the engine accepts, \
         which it refuses outright rather than clamping"
    );
    assert_eq!(clayspace_model::division_limit(Representation::Voxel), None);
}

// -- the manipulator ---------------------------------------------------------
//
// What makes selecting more than one control point worth having. Dragging
// points one at a time needs no manipulator; turning a whole face of the cage
// cannot be done without one.

/// Selects every control point above the middle on an axis.
fn select_the_far_face(document: &mut ClayDocument, axis: usize) -> Vec<usize> {
    let cage = document.lattice();
    let face: Vec<usize> = cage
        .points
        .iter()
        .enumerate()
        .filter(|(_, point)| point[axis] > 0.0)
        .map(|(index, _)| index)
        .collect();
    for index in &face {
        document.toggle_lattice_point(*index);
    }
    face
}

#[test]
fn a_selection_is_built_a_point_at_a_time() {
    let mut document = meshed();
    document.begin_lattice([2, 2, 2]).expect("a cage");

    // One click replaces the selection.
    document.select_lattice_point(Some(3));
    assert_eq!(document.lattice().selection, vec![3]);
    document.select_lattice_point(Some(5));
    assert_eq!(document.lattice().selection, vec![5]);

    // A modifier-click adds without disturbing the rest, and takes back out.
    document.toggle_lattice_point(1);
    document.toggle_lattice_point(7);
    assert_eq!(
        document.lattice().selection,
        vec![1, 5, 7],
        "the selection is not kept in order, so the pivot would depend on \
         which corner was clicked first"
    );
    document.toggle_lattice_point(5);
    assert_eq!(document.lattice().selection, vec![1, 7]);

    // And clearing means clearing.
    document.select_lattice_point(None);
    assert!(document.lattice().selection.is_empty());
    assert_eq!(
        document.lattice().pivot(),
        None,
        "an empty selection has a manipulator with nothing to manipulate"
    );
}

#[test]
fn a_selection_box_takes_a_whole_set_at_once() {
    // What a rubber band drawn across the viewport leaves behind. Not a loop
    // over the one-point call, which would keep only the last of them, and not
    // one over the toggle, which would take back whatever was already held —
    // a box says *these*, not *these as well as the opposite of what you had*.
    let mut document = meshed();
    document.begin_lattice([2, 2, 2]).expect("a cage");

    document.select_lattice_point(Some(2));
    document.select_lattice_points(&[5, 1, 5, 7]);
    assert_eq!(
        document.lattice().selection,
        vec![1, 5, 7],
        "a box did not replace what was held, in order and without repeats"
    );

    // A point the cage does not have is not a point. A box is drawn in screen
    // space and resolved against whatever cage is up, and a stale index must
    // not put the manipulator somewhere the sculptor cannot see.
    let count = document.lattice().points.len();
    document.select_lattice_points(&[0, count + 4]);
    assert_eq!(document.lattice().selection, vec![0]);

    // An empty box is a click on nothing, which clears.
    document.select_lattice_points(&[]);
    assert!(document.lattice().selection.is_empty());
    assert_eq!(document.lattice().pivot(), None);
}

#[test]
fn a_box_round_a_face_gives_the_manipulator_that_face() {
    // The point of gathering several at once: turning and scaling act about
    // the middle of the selection and refuse a selection of one, so a whole
    // face has to be reachable in a gesture rather than in four clicks.
    let mut document = meshed();
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let cage = document.lattice();
    let face: Vec<usize> = cage
        .points
        .iter()
        .enumerate()
        .filter(|(_, point)| point[1] > 0.0)
        .map(|(index, _)| index)
        .collect();

    document.select_lattice_points(&face);
    let cage = document.lattice();
    assert_eq!(cage.selection, face);
    assert!(
        cage.can_transform(),
        "a face gathered in one gesture cannot be turned"
    );
    let pivot = cage.pivot().expect("a face has a middle");
    assert!(
        pivot[1] > 0.0,
        "the manipulator sat at {pivot:?} rather than on the face"
    );
}

#[test]
fn the_manipulator_moves_a_whole_face() {
    let mut document = meshed();
    let before = top(&mut document);
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let face = select_the_far_face(&mut document, 1);
    assert_eq!(face.len(), 4, "the top of a cube is four corners");

    let pivot = document
        .lattice()
        .pivot()
        .expect("a selection has a middle");
    document.set_gizmo_mode(GizmoMode::Move);
    document.begin_gizmo_drag(GizmoHandle::Axis(1), pivot, LOOKING_DOWN_Z);
    document
        .drag_gizmo([pivot[0], pivot[1] + 0.5, pivot[2]], false)
        .expect("the drag was refused");
    document.end_gizmo_drag();
    document.apply_lattice().expect("the cage was refused");

    let after = top(&mut document);
    assert!(
        (after - (before + 0.5)).abs() < 0.05,
        "the top went from {before} to {after} after its whole face was moved \
         up by 0.5 through the manipulator"
    );
}

#[test]
fn an_axis_drag_does_not_wander_off_its_axis() {
    // The whole difference between an arrow and the centre handle: a person
    // pulling the green arrow means "up", not "up and a little sideways
    // because my hand drifted".
    let mut document = meshed();
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let before = document.lattice().points.clone();
    select_the_far_face(&mut document, 1);

    let pivot = document.lattice().pivot().expect("a middle");
    document.set_gizmo_mode(GizmoMode::Move);
    document.begin_gizmo_drag(GizmoHandle::Axis(1), pivot, LOOKING_DOWN_Z);
    // Deliberately crooked: the pointer wanders in x and z as well.
    document
        .drag_gizmo([pivot[0] + 0.4, pivot[1] + 0.5, pivot[2] - 0.3], false)
        .expect("the drag was refused");

    let after = document.lattice().points;
    for (index, (was, now)) in before.iter().zip(&after).enumerate() {
        if was[1] <= 0.0 {
            assert_eq!(was, now, "point {index} moved and was not selected");
            continue;
        }
        assert!(
            (now[0] - was[0]).abs() < 1e-5 && (now[2] - was[2]).abs() < 1e-5,
            "point {index} went from {was:?} to {now:?}, which is off the axis \
             that was grabbed"
        );
        assert!((now[1] - was[1] - 0.5).abs() < 1e-5);
    }
}

#[test]
fn a_rotation_turns_the_selection_about_its_own_middle() {
    let mut document = meshed();
    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");
    let before = document.lattice().points.clone();

    document.set_gizmo_mode(GizmoMode::Rotate);
    // A quarter turn about y: from +x to +z, in the plane the ring lies in.
    document.begin_gizmo_drag(
        GizmoHandle::Axis(1),
        [pivot[0] + 1.0, pivot[1], pivot[2]],
        LOOKING_DOWN_Z,
    );
    document
        .drag_gizmo([pivot[0], pivot[1], pivot[2] + 1.0], false)
        .expect("the drag was refused");

    let after = document.lattice().points;
    for (index, (was, now)) in before.iter().zip(&after).enumerate() {
        if was[1] <= 0.0 {
            continue;
        }
        // A quarter turn about y takes (x, z) to (z, -x), measured from the
        // pivot. And y is untouched, which is what "about that axis" means.
        let (x, z) = (was[0] - pivot[0], was[2] - pivot[2]);
        assert!(
            (now[0] - (pivot[0] - z)).abs() < 1e-4 && (now[2] - (pivot[2] + x)).abs() < 1e-4,
            "point {index} went from {was:?} to {now:?} on a quarter turn \
             about y through {pivot:?}"
        );
        assert!(
            (now[1] - was[1]).abs() < 1e-5,
            "a turn about y moved point {index} along y"
        );
    }
}

#[test]
fn a_scale_spreads_the_selection_about_its_middle() {
    let mut document = meshed();
    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");
    let before = document.lattice().points.clone();

    document.set_gizmo_mode(GizmoMode::Scale);
    document.begin_gizmo_drag(
        GizmoHandle::Centre,
        [pivot[0] + 1.0, pivot[1], pivot[2]],
        LOOKING_DOWN_Z,
    );
    document
        .drag_gizmo([pivot[0] + 2.0, pivot[1], pivot[2]], false)
        .expect("the drag was refused");

    let after = document.lattice().points;
    for (was, now) in before.iter().zip(&after) {
        if was[1] <= 0.0 {
            continue;
        }
        for axis in 0..3 {
            let want = pivot[axis] + (was[axis] - pivot[axis]) * 2.0;
            assert!(
                (now[axis] - want).abs() < 1e-4,
                "a uniform scale of two put {was:?} at {now:?}"
            );
        }
    }
}

#[test]
fn a_manipulator_drag_is_resolved_from_its_anchor_every_time() {
    // Rather than accumulated. Transforming what the last frame produced
    // compounds a rotation into a spiral and a scale into a runaway, and a
    // stutter in the pointer would show up as a jump in the form.
    let mut document = meshed();
    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");

    document.set_gizmo_mode(GizmoMode::Move);
    document.begin_gizmo_drag(GizmoHandle::Axis(1), pivot, LOOKING_DOWN_Z);
    // A pointer wandering on its way: an intermediate destination, then the
    // one it settles on, then the same one again as a held pointer that has
    // stopped moving keeps sending.
    for at in [0.2, 0.5, 0.5] {
        document
            .drag_gizmo([pivot[0], pivot[1] + at, pivot[2]], false)
            .expect("the drag was refused");
    }
    let wandered = document.lattice().points.clone();
    document.end_gizmo_drag();

    // The same gesture, straight to where it ended.
    let mut direct = meshed();
    direct.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut direct, 1);
    direct.set_gizmo_mode(GizmoMode::Move);
    direct.begin_gizmo_drag(GizmoHandle::Axis(1), pivot, LOOKING_DOWN_Z);
    direct
        .drag_gizmo([pivot[0], pivot[1] + 0.5, pivot[2]], false)
        .expect("the drag was refused");

    assert_eq!(
        wandered,
        direct.lattice().points,
        "a drag that wandered on its way did not land where one that went \
         straight there did, so the transform is accumulating rather than \
         being resolved from its anchor — which compounds a rotation into a \
         spiral and a scale into a runaway"
    );
}

#[test]
fn the_manipulator_does_nothing_with_nothing_selected() {
    // A press is not an edit, and a widget with no selection has nothing to
    // act on — reaching for the offsets anyway would move point zero.
    let mut document = meshed();
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let before = document.lattice().points.clone();

    document.begin_gizmo_drag(GizmoHandle::Axis(1), [0.0; 3], LOOKING_DOWN_Z);
    document
        .drag_gizmo([0.0, 5.0, 0.0], false)
        .expect("the drag was refused");
    assert_eq!(document.lattice().points, before);
    assert!(!document.lattice().touched);
}

// -- seeing it while it happens ----------------------------------------------
//
// A cage that showed nothing until it was applied made the sculptor aim blind:
// they set every corner, pressed Deformar, looked at the result and started
// again. The forward route deforms vertices they already have, so a preview is
// one pass and taking it back is one more.

#[test]
fn the_bend_is_shown_while_the_cage_is_dragged() {
    let mut document = meshed();
    let rest = top(&mut document);
    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");

    document.set_gizmo_mode(GizmoMode::Move);
    document.begin_gizmo_drag(GizmoHandle::Axis(1), pivot, LOOKING_DOWN_Z);
    document
        .drag_gizmo([pivot[0], pivot[1] + 0.5, pivot[2]], false)
        .expect("the drag was refused");

    // Mid-gesture, with nothing applied and nothing banked.
    let shown = top(&mut document);
    assert!(
        (shown - (rest + 0.5)).abs() < 0.05,
        "the form still stands at {shown} from {rest} while the cage is being \
         dragged, so a sculptor is aiming blind"
    );
    assert!(
        document.history().depth == 0 || document.lattice().active,
        "a preview banked an edit"
    );
}

#[test]
fn a_preview_does_not_compound_across_frames() {
    // The lattice is absolute — offsets from rest, evaluated against the
    // original vertices — so laying it over a surface a previous frame already
    // bent doubles the deformation on every pointer move. What the last
    // preview did is taken back first.
    let mut stepped = meshed();
    stepped.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut stepped, 1);
    let pivot = stepped.lattice().pivot().expect("a middle");
    stepped.set_gizmo_mode(GizmoMode::Move);
    stepped.begin_gizmo_drag(GizmoHandle::Axis(1), pivot, LOOKING_DOWN_Z);
    // Twenty frames, as a pointer crossing the viewport would send.
    for step in 1..=20 {
        let by = step as f32 / 20.0 * 0.5;
        stepped
            .drag_gizmo([pivot[0], pivot[1] + by, pivot[2]], false)
            .expect("the drag was refused");
    }
    let after_many = top(&mut stepped);

    let mut once = meshed();
    once.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut once, 1);
    once.set_gizmo_mode(GizmoMode::Move);
    once.begin_gizmo_drag(GizmoHandle::Axis(1), pivot, LOOKING_DOWN_Z);
    once.drag_gizmo([pivot[0], pivot[1] + 0.5, pivot[2]], false)
        .expect("the drag was refused");

    assert!(
        (after_many - top(&mut once)).abs() < 1e-3,
        "twenty frames of one drag left the form at {after_many} where one \
         frame left it at {}, so each frame is bending what the last one bent",
        top(&mut once)
    );
}

#[test]
fn the_viewport_is_told_to_look_again_on_every_drag() {
    // A mesh layer is not in the brick cache, so nothing else about this edit
    // would say the surface had moved — and the viewport uploads only when the
    // revision changes.
    let mut document = meshed();
    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");
    document.set_gizmo_mode(GizmoMode::Move);
    document.begin_gizmo_drag(GizmoHandle::Axis(1), pivot, LOOKING_DOWN_Z);

    let mut seen = std::collections::BTreeSet::new();
    for step in 1..=5 {
        document
            .drag_gizmo([pivot[0], pivot[1] + step as f32 * 0.1, pivot[2]], false)
            .expect("the drag was refused");
        seen.insert(document.mesh_revision());
    }
    assert_eq!(
        seen.len(),
        5,
        "five drags produced {} distinct revisions, so the viewport would draw \
         a form that has visibly moved as though it had not",
        seen.len()
    );
}

#[test]
fn a_previewed_cage_is_still_one_undo() {
    // Every drag replaces the last rather than adding to it, so bending a form
    // is one undo however many times a corner was adjusted on the way.
    let mut document = meshed();
    let rest = top(&mut document);
    let before = document.history().depth;
    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");
    document.set_gizmo_mode(GizmoMode::Move);
    document.begin_gizmo_drag(GizmoHandle::Axis(1), pivot, LOOKING_DOWN_Z);
    for step in 1..=10 {
        document
            .drag_gizmo([pivot[0], pivot[1] + step as f32 * 0.05, pivot[2]], false)
            .expect("the drag was refused");
    }
    document.end_gizmo_drag();
    document.apply_lattice().expect("the cage was refused");

    assert_eq!(
        document.history().depth,
        before + 1,
        "ten drags left more than one entry on the stack"
    );
    let applied = top(&mut document);
    assert!(
        (applied - (rest + 0.5)).abs() < 0.05,
        "applying after a preview left the form at {applied} rather than where \
         the preview was showing it"
    );
    document.undo().expect("undo");
    assert!(
        (top(&mut document) - rest).abs() < 1e-3,
        "one undo did not give the form back"
    );
}

#[test]
fn cancelling_takes_the_preview_back() {
    let mut document = meshed();
    let rest = top(&mut document);
    // Not zero: the fixture crossed a sphere into a mesh, and both are edits.
    let before = document.history().depth;
    document.begin_lattice([2, 2, 2]).expect("a cage");
    lift(&mut document, 1, 0.5);

    // It really was showing something, or the assertion below passes for the
    // wrong reason.
    assert!(
        top(&mut document) > rest + 0.4,
        "there was no preview to take back"
    );
    document.cancel_lattice();
    assert!(
        (top(&mut document) - rest).abs() < 1e-4,
        "abandoning a cage left the form bent at {}, which is the opposite of \
         what abandoning a gesture means everywhere else here",
        top(&mut document)
    );
    assert_eq!(
        document.history().depth,
        before,
        "cancelling banked an edit"
    );
}

#[test]
fn a_field_cage_moves_its_points_without_previewing_the_surface() {
    // No preview there, and deliberately: the field route writes a lattice
    // deformer into the document as an undoable edit and refills the layer's
    // whole brick region — 68.8 ms measured for one apply on the starting
    // form, against 11.2 ms for a mesh preview on 62,576 vertices. That is not
    // a thing to do on every pointer move, so the cage moves live and the
    // surface follows when it is applied.
    let mut document = sphere();
    let rest = reach(&document);
    document.begin_lattice([4, 4, 4]).expect("a cage");
    lift(&mut document, 2, 0.5);

    assert!(
        document.lattice().touched,
        "the cage's own points did not move"
    );
    assert!(
        (reach(&document) - rest).abs() < 1e-3,
        "a field cage previewed the surface at {} from {rest}",
        reach(&document)
    );
    document.apply_lattice().expect("the cage was refused");
    assert!(
        reach(&document) > rest + 0.3,
        "applying a field cage did not move the surface"
    );
}

/// Where a pointer travelling in the drag plane actually lands, which is the
/// step the application performs with a ray and the manipulator's own tests
/// skip by handing world points straight in.
fn swept_across(normal: [f32; 3], pivot: [f32; 3], quarter: bool) -> [f32; 3] {
    let (across, other) = clayspace_model::perpendicular_frame(normal);
    let at = if quarter { other } else { across };
    std::array::from_fn(|i| pivot[i] + at[i])
}

#[test]
fn every_ring_turns_the_cage_when_dragged_across_the_screen() {
    // The regression. The drag plane was chosen to *contain* the axis, which
    // is right for a slide and exactly wrong for a turn: a ring lies in the
    // plane perpendicular to what it turns about. Two of the three rings moved
    // the cage by nothing at all however far the pointer went, and only the
    // one whose axis pointed at the camera worked.
    //
    // Driven through `drag_plane` here, which is the step that was wrong —
    // the tests above hand world points straight to the document and could
    // not see it.
    let mut document = meshed();
    // A camera in front, looking down −z, so `facing` points back at it.
    let facing = [0.0, 0.0, 1.0];

    for index in 0..3 {
        document.begin_lattice([2, 2, 2]).expect("a cage");
        select_the_far_face(&mut document, 1);
        let pivot = document.lattice().pivot().expect("a middle");
        let before = document.lattice().points.clone();

        let handle = GizmoHandle::Axis(index);
        document.set_gizmo_mode(GizmoMode::Rotate);
        let normal = clayspace_model::drag_plane(GizmoMode::Rotate, handle, facing, facing);
        document.begin_gizmo_drag(handle, swept_across(normal, pivot, false), facing);
        document
            .drag_gizmo(swept_across(normal, pivot, true), false)
            .expect("the drag was refused");

        let after = document.lattice().points;
        let moved = before
            .iter()
            .zip(&after)
            .map(|(was, now)| {
                (0..3)
                    .map(|i| (now[i] - was[i]).powi(2))
                    .sum::<f32>()
                    .sqrt()
            })
            .fold(0.0f32, f32::max);
        assert!(
            moved > 0.1,
            "the ring about axis {index} moved the cage by {moved} on a \
             quarter turn across the screen"
        );
        document.cancel_lattice();
    }
}

#[test]
fn the_outer_ring_turns_the_cage_too() {
    let mut document = meshed();
    // A camera off the world axes, so this cannot pass by accident on one.
    let view = {
        let raw = [0.4f32, 0.5, 0.77];
        let length = raw.iter().map(|c| c * c).sum::<f32>().sqrt();
        std::array::from_fn(|i| raw[i] / length)
    };
    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");
    let before = document.lattice().points.clone();

    document.set_gizmo_mode(GizmoMode::Rotate);
    let normal = clayspace_model::drag_plane(GizmoMode::Rotate, GizmoHandle::View, view, view);
    document.begin_gizmo_drag(GizmoHandle::View, swept_across(normal, pivot, false), view);
    document
        .drag_gizmo(swept_across(normal, pivot, true), false)
        .expect("the drag was refused");

    let after = document.lattice().points;
    let moved = before
        .iter()
        .zip(&after)
        .map(|(was, now)| {
            (0..3)
                .map(|i| (now[i] - was[i]).powi(2))
                .sum::<f32>()
                .sqrt()
        })
        .fold(0.0f32, f32::max);
    assert!(moved > 0.1, "the outer ring moved the cage by {moved}");
}

#[test]
fn the_axis_pointing_at_the_camera_can_still_be_scaled() {
    // The mirror of the ring bug, in the same line: when the axis pointed at
    // the eye the plane degenerated to the one facing the camera, which puts
    // the anchor's component along the axis at zero — and a scale divides by
    // that, so the handle went dead.
    let mut document = meshed();
    let facing = [0.0, 0.0, 1.0];
    let handle = GizmoHandle::Axis(2);

    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");
    let before = document.lattice().points.clone();

    document.set_gizmo_mode(GizmoMode::Scale);
    let normal = clayspace_model::drag_plane(GizmoMode::Scale, handle, facing, facing);
    // Grabbed a unit out along the axis and pulled to twice that.
    let anchor: [f32; 3] = std::array::from_fn(|i| pivot[i] + [0.0, 0.0, 1.0][i]);
    let to: [f32; 3] = std::array::from_fn(|i| pivot[i] + [0.0, 0.0, 2.0][i]);
    let along: f32 = (0..3).map(|i| normal[i] * [0.0, 0.0, 1.0][i]).sum();
    assert!(
        along.abs() < 1e-3,
        "the drag plane no longer contains the axis facing the eye"
    );

    document.begin_gizmo_drag(handle, anchor, facing);
    document
        .drag_gizmo(to, false)
        .expect("the drag was refused");

    let after = document.lattice().points;
    let moved = before
        .iter()
        .zip(&after)
        .map(|(was, now)| {
            (0..3)
                .map(|i| (now[i] - was[i]).powi(2))
                .sum::<f32>()
                .sqrt()
        })
        .fold(0.0f32, f32::max);
    assert!(
        moved > 0.1,
        "a scale along the axis facing the eye moved {moved}"
    );
}
