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
use clayspace_model::{
    Direction, GizmoHandle, GizmoMode, LatticeModel, Representation, SculptModel,
};

fn sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

fn meshed() -> Option<ClayDocument> {
    let mut document = sphere()?;
    document.convert_layer(Direction::SdfToMesh, 0.03, 0).ok()?;
    Some(document)
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
    let Some(mut document) = meshed() else {
        return;
    };
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
    let Some(mut document) = meshed() else {
        return;
    };
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
    let Some(mut document) = sphere() else {
        return;
    };
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
    let Some(mut document) = meshed() else {
        return;
    };
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
    let Some(mut document) = meshed() else {
        return;
    };
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
    let Some(mut document) = meshed() else {
        return;
    };
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
    let Some(policy) = BackendPolicy::discover(None).ok() else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy) else {
        return;
    };
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
    let Some(mut mesh) = meshed() else {
        return;
    };
    mesh.begin_lattice([32, 32, 32]).expect("a cage");
    assert_eq!(mesh.lattice().divisions, [32; 3]);
    assert_eq!(mesh.lattice().points.len(), 32 * 32 * 32);

    let Some(mut field) = sphere() else {
        return;
    };
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
    let Some(mut document) = meshed() else {
        return;
    };
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
fn the_manipulator_moves_a_whole_face() {
    let Some(mut document) = meshed() else {
        return;
    };
    let before = top(&mut document);
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let face = select_the_far_face(&mut document, 1);
    assert_eq!(face.len(), 4, "the top of a cube is four corners");

    let pivot = document
        .lattice()
        .pivot()
        .expect("a selection has a middle");
    document.set_gizmo_mode(GizmoMode::Move);
    document.begin_gizmo_drag(GizmoHandle::Axis(1), pivot);
    document
        .drag_gizmo([pivot[0], pivot[1] + 0.5, pivot[2]])
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
    let Some(mut document) = meshed() else {
        return;
    };
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let before = document.lattice().points.clone();
    select_the_far_face(&mut document, 1);

    let pivot = document.lattice().pivot().expect("a middle");
    document.set_gizmo_mode(GizmoMode::Move);
    document.begin_gizmo_drag(GizmoHandle::Axis(1), pivot);
    // Deliberately crooked: the pointer wanders in x and z as well.
    document
        .drag_gizmo([pivot[0] + 0.4, pivot[1] + 0.5, pivot[2] - 0.3])
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
    let Some(mut document) = meshed() else {
        return;
    };
    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");
    let before = document.lattice().points.clone();

    document.set_gizmo_mode(GizmoMode::Rotate);
    // A quarter turn about y: from +x to +z, in the plane the ring lies in.
    document.begin_gizmo_drag(GizmoHandle::Axis(1), [pivot[0] + 1.0, pivot[1], pivot[2]]);
    document
        .drag_gizmo([pivot[0], pivot[1], pivot[2] + 1.0])
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
    let Some(mut document) = meshed() else {
        return;
    };
    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");
    let before = document.lattice().points.clone();

    document.set_gizmo_mode(GizmoMode::Scale);
    document.begin_gizmo_drag(GizmoHandle::Centre, [pivot[0] + 1.0, pivot[1], pivot[2]]);
    document
        .drag_gizmo([pivot[0] + 2.0, pivot[1], pivot[2]])
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
    let Some(mut document) = meshed() else {
        return;
    };
    document.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut document, 1);
    let pivot = document.lattice().pivot().expect("a middle");

    document.set_gizmo_mode(GizmoMode::Move);
    document.begin_gizmo_drag(GizmoHandle::Axis(1), pivot);
    // A pointer wandering on its way: an intermediate destination, then the
    // one it settles on, then the same one again as a held pointer that has
    // stopped moving keeps sending.
    for at in [0.2, 0.5, 0.5] {
        document
            .drag_gizmo([pivot[0], pivot[1] + at, pivot[2]])
            .expect("the drag was refused");
    }
    let wandered = document.lattice().points.clone();
    document.end_gizmo_drag();

    // The same gesture, straight to where it ended.
    let Some(mut direct) = meshed() else {
        return;
    };
    direct.begin_lattice([2, 2, 2]).expect("a cage");
    select_the_far_face(&mut direct, 1);
    direct.set_gizmo_mode(GizmoMode::Move);
    direct.begin_gizmo_drag(GizmoHandle::Axis(1), pivot);
    direct
        .drag_gizmo([pivot[0], pivot[1] + 0.5, pivot[2]])
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
    let Some(mut document) = meshed() else {
        return;
    };
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let before = document.lattice().points.clone();

    document.begin_gizmo_drag(GizmoHandle::Axis(1), [0.0; 3]);
    document
        .drag_gizmo([0.0, 5.0, 0.0])
        .expect("the drag was refused");
    assert_eq!(document.lattice().points, before);
    assert!(!document.lattice().touched);
}
