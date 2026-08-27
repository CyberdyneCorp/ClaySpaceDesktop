//! Placing a shape in a layer and coming back to it.
//!
//! The session half of the boolean workflow: what the bridge tests prove the
//! engine does, these prove the application does *through* it — including the
//! part the engine cannot help with, which is remembering where an object is
//! when history moves it.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Combine, CombineSettings, DocumentModel, ObjectModel, Representation, SceneModel, SculptModel,
    Shape,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// A second document, for comparing one gesture against another.
fn document_fresh() -> ClayDocument {
    document()
}

/// Opens a document from a path, on the same terms `document` builds one.
fn document_at(path: &std::path::Path) -> ClayDocument {
    let mut opened = document();
    opened.open(path).expect("open");
    opened
}

fn subtracting() -> CombineSettings {
    CombineSettings {
        op: Combine::Subtract,
        ..CombineSettings::default()
    }
}

/// Whether the surface encloses a point.
fn inside(document: &ClayDocument, at: [f32; 3]) -> bool {
    document
        .document()
        .eval_points(None, &[at])
        .is_ok_and(|values| values[0] < 0.0)
}

#[test]
fn a_placed_object_is_listed_and_selected() {
    let mut document = document();
    let id = document
        .place_object(
            Shape::Cylinder,
            &Shape::Cylinder.defaults(),
            [0.5, 0.0, 0.0],
            subtracting(),
        )
        .expect("place");

    assert_eq!(document.selected_object(), Some(id));
    let objects = document.objects();
    assert!(
        objects.iter().any(|object| object.id == id),
        "the object should be listed among {} others",
        objects.len()
    );
}

/// The starting form is a placed sphere and always was; nothing but the
/// absence of a selection model made it special.
#[test]
fn the_starting_form_is_an_object_too() {
    let mut document = document();
    assert_eq!(
        document.objects().len(),
        1,
        "the opening sphere should be listed as the object it is"
    );
}

/// What a stroke leaves behind is not a row in the object list.
#[test]
fn a_sculpting_stroke_is_not_an_object() {
    let mut document = document();
    let before = document.objects().len();
    document
        .apply_stroke(
            clayspace_model::ToolKind::Padrao,
            clayspace_model::BrushSettings::default(),
            &[clayspace_model::GestureSample {
                position: [0.0, 1.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("stroke");

    assert_eq!(
        document.objects().len(),
        before,
        "a stroke added a row to the object list"
    );
}

/// Along Y, which the X mirror maps to itself — see
/// `a_placed_object_is_mirrored_like_everything_else` for why that matters.
#[test]
fn a_moved_object_moves_its_cavity() {
    let mut document = document();
    let id = document
        .place_object(Shape::Sphere, &[0.4], [0.0, 0.9, 0.0], subtracting())
        .expect("place");
    assert!(!inside(&document, [0.0, 0.9, 0.0]), "the cavity is cut");

    document
        .set_object_transform(id, [0.0, -0.9, 0.0], [0.0, 1.0, 0.0], 0.0, 1.0)
        .expect("move");

    assert!(inside(&document, [0.0, 0.9, 0.0]), "the old cavity closed");
    assert!(!inside(&document, [0.0, -0.9, 0.0]), "and moved with it");
}

/// The application mirrors its layers in X, "as the design asks for", and an
/// object is an item in a layer like any other — so one placed off the plane
/// cuts on both sides. Worth a test of its own because it is the behaviour a
/// sculptor actually meets, and because it is why the manipulator cannot take
/// an object's position from the engine's influence bound: that box covers
/// both copies and centres between them.
#[test]
fn a_placed_object_is_mirrored_like_everything_else() {
    let mut document = document();
    document
        .place_object(Shape::Sphere, &[0.4], [0.9, 0.0, 0.0], subtracting())
        .expect("place");

    assert!(!inside(&document, [0.9, 0.0, 0.0]), "cut where it was put");
    assert!(
        !inside(&document, [-0.9, 0.0, 0.0]),
        "and at its reflection"
    );
}

#[test]
fn an_operation_is_editable_after_placement() {
    let mut document = document();
    let id = document
        .place_object(
            Shape::Sphere,
            &[0.4],
            [1.0, 0.0, 0.0],
            CombineSettings::default(),
        )
        .expect("place");
    assert!(
        inside(&document, [1.2, 0.0, 0.0]),
        "added material is there"
    );

    document
        .set_object_combine(id, subtracting())
        .expect("re-op");
    assert!(
        !inside(&document, [1.0, 0.0, 0.0]),
        "the same node now cuts"
    );

    let listed = document.objects();
    let object = listed.iter().find(|o| o.id == id).expect("still listed");
    assert_eq!(object.combine.op, Combine::Subtract, "and the list says so");
}

#[test]
fn a_shape_is_exchangeable_without_losing_where_it_is() {
    let mut document = document();
    let id = document
        .place_object(
            Shape::Box,
            &Shape::Box.defaults(),
            [0.9, 0.0, 0.0],
            subtracting(),
        )
        .expect("place");
    document
        .set_object_shape(id, Shape::Cylinder, &[0.25, 2.0])
        .expect("exchange");

    assert!(
        !inside(&document, [0.9, 0.0, 0.0]),
        "the new shape cuts here"
    );
    let listed = document.objects();
    let object = listed.iter().find(|o| o.id == id).expect("still listed");
    assert_eq!(object.source.shape(), Some(Shape::Cylinder));
}

#[test]
fn removing_an_object_closes_what_it_cut() {
    let mut document = document();
    let id = document
        .place_object(Shape::Sphere, &[0.5], [0.8, 0.0, 0.0], subtracting())
        .expect("place");
    assert!(!inside(&document, [0.8, 0.0, 0.0]));

    document.remove_object(id).expect("remove");
    assert!(inside(&document, [0.8, 0.0, 0.0]), "the cavity is gone");
    assert!(
        !document.objects().iter().any(|object| object.id == id),
        "and so is the row"
    );
    assert_eq!(document.selected_object(), None, "and the selection");
}

// -- history ----------------------------------------------------------------

/// The one the readback gap makes interesting. The engine takes the transform
/// back and cannot tell the table it did, so the table follows by depth.
#[test]
fn undoing_a_move_takes_the_table_back_with_it() {
    let mut document = document();
    let id = document
        .place_object(Shape::Sphere, &[0.4], [0.0, 0.9, 0.0], subtracting())
        .expect("place");
    document
        .set_object_transform(id, [0.0, -0.9, 0.0], [0.0, 1.0, 0.0], 0.0, 1.0)
        .expect("move");

    document.undo().expect("undo the move");

    let listed = document.objects();
    let object = listed.iter().find(|o| o.id == id).expect("still listed");
    assert!(
        (object.position[1] - 0.9).abs() < 0.2,
        "the table should have followed the engine back, reports {:?}",
        object.position
    );
    assert!(
        !inside(&document, [0.0, 0.9, 0.0]),
        "and the cavity is back where it was"
    );
}

#[test]
fn undoing_a_placement_takes_the_row_away() {
    let mut document = document();
    let before = document.objects().len();
    let id = document
        .place_object(
            Shape::Box,
            &Shape::Box.defaults(),
            [0.9, 0.0, 0.0],
            subtracting(),
        )
        .expect("place");
    assert_eq!(document.objects().len(), before + 1);

    document.undo().expect("undo");
    assert_eq!(
        document.objects().len(),
        before,
        "the row should go with the object"
    );
    assert_ne!(document.selected_object(), Some(id), "and the selection");
}

#[test]
fn redoing_a_move_puts_the_table_back_where_it_was() {
    let mut document = document();
    let id = document
        .place_object(Shape::Sphere, &[0.4], [0.0, 0.9, 0.0], subtracting())
        .expect("place");
    document
        .set_object_transform(id, [0.0, -0.9, 0.0], [0.0, 1.0, 0.0], 0.0, 1.0)
        .expect("move");
    document.undo().expect("undo");
    document.redo().expect("redo");

    let listed = document.objects();
    let object = listed.iter().find(|o| o.id == id).expect("still listed");
    assert!(
        (object.position[1] + 0.9).abs() < 0.2,
        "the table should have followed the redo forward, reports {:?}",
        object.position
    );
}

/// A stroke raises the engine's depth and touches no object, and the table has
/// to stay where it is across one.
#[test]
fn a_stroke_between_object_edits_does_not_disturb_the_table() {
    let mut document = document();
    let id = document
        .place_object(Shape::Sphere, &[0.4], [0.0, 0.9, 0.0], subtracting())
        .expect("place");
    document
        .apply_stroke(
            clayspace_model::ToolKind::Padrao,
            clayspace_model::BrushSettings::default(),
            &[clayspace_model::GestureSample {
                position: [0.0, 1.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("stroke");

    let listed = document.objects();
    let object = listed.iter().find(|o| o.id == id).expect("still listed");
    assert!(
        (object.position[1] - 0.9).abs() < 0.2,
        "a stroke moved the object's recorded position to {:?}",
        object.position
    );
}

// -- refusals ---------------------------------------------------------------

#[test]
fn a_grid_has_nowhere_to_put_an_object() {
    let mut document = document();
    document
        .add_voxel_layer("Voxels", 0.05)
        .expect("add a grid");
    assert_eq!(document.active_representation(), Representation::Voxel);

    let refusal = document
        .place_object(
            Shape::Box,
            &Shape::Box.defaults(),
            [0.0; 3],
            CombineSettings::default(),
        )
        .expect_err("a grid holds no ordered list of items");
    // The refusal names where an object does apply rather than restating one
    // representation's answer for all of them.
    assert!(
        refusal.to_string().to_lowercase().contains("sdf")
            || refusal.to_string().contains("clay_layer_add_item"),
        "the refusal should say where an object can live: {refusal}"
    );
}

#[test]
fn an_object_that_is_gone_refuses_rather_than_panicking() {
    let mut document = document();
    let id = document
        .place_object(Shape::Sphere, &[0.3], [0.9, 0.0, 0.0], subtracting())
        .expect("place");
    document.remove_object(id).expect("remove");

    assert!(document
        .set_object_transform(id, [0.0; 3], [0.0, 1.0, 0.0], 0.0, 1.0)
        .is_err());
    assert!(document.remove_object(id).is_err());
}

// -- across a save and a reopen ---------------------------------------------

#[test]
fn a_placed_object_survives_a_reopen() {
    let mut document = document();
    let path = std::env::temp_dir().join("clayspace-objects-reopen.clay");
    let _ = std::fs::remove_file(&path);

    let id = document
        .place_object(
            Shape::Cylinder,
            &[0.25, 0.8],
            [0.0, 0.6, 0.0],
            subtracting(),
        )
        .expect("place");
    document
        .set_object_transform(id, [0.0, 0.6, 0.0], [0.0, 1.0, 0.0], 0.0, 1.5)
        .expect("scale it up");
    document.save(&path).expect("save");

    let mut reopened = document_at(&path);
    let listed = reopened.objects();
    let object = listed
        .iter()
        .find(|object| object.id == id)
        .expect("the object should come back");

    assert_eq!(object.source.shape(), Some(Shape::Cylinder));
    assert_eq!(object.combine.op, Combine::Subtract);
    assert!((object.scale - 1.5).abs() < 1e-4, "scale {}", object.scale);
    assert!(
        (object.position[1] - 0.6).abs() < 1e-4,
        "position {:?}",
        object.position
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(clayspace_engine::objects::sidecar_for(&path));
}

/// A document whose side-car is gone still opens and still sculpts. What is
/// lost is which of its shapes can be picked up again — which is the safe
/// direction to fail in, and worth saying in a test rather than in a comment.
#[test]
fn a_document_without_its_sidecar_still_opens() {
    let mut document = document();
    let path = std::env::temp_dir().join("clayspace-objects-no-sidecar.clay");
    let _ = std::fs::remove_file(&path);

    document
        .place_object(
            Shape::Box,
            &Shape::Box.defaults(),
            [0.0, 0.6, 0.0],
            subtracting(),
        )
        .expect("place");
    document.save(&path).expect("save");
    std::fs::remove_file(clayspace_engine::objects::sidecar_for(&path)).expect("drop the side-car");

    let mut reopened = document_at(&path);
    assert!(
        reopened.objects().is_empty(),
        "without the side-car nothing is offered as an object"
    );
    // And the sculpture is all there: the box still cuts.
    assert!(
        !inside(&reopened, [0.0, 0.6, 0.0]),
        "the cut is in the .clay"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_malformed_line_costs_one_object_rather_than_the_file() {
    let mut document = document();
    let path = std::env::temp_dir().join("clayspace-objects-malformed.clay");
    let _ = std::fs::remove_file(&path);
    document
        .place_object(Shape::Sphere, &[0.3], [0.0, 0.6, 0.0], subtracting())
        .expect("place");
    document
        .place_object(
            Shape::Box,
            &Shape::Box.defaults(),
            [0.0, -0.6, 0.0],
            subtracting(),
        )
        .expect("place another");
    document.save(&path).expect("save");

    let sidecar = clayspace_engine::objects::sidecar_for(&path);
    let text = std::fs::read_to_string(&sidecar).expect("read");
    let mut lines: Vec<&str> = text.lines().collect();
    // Damage the second row — the starting form is the first.
    lines[1] = "1 2 not-a-shape";
    std::fs::write(&sidecar, lines.join("\n")).expect("write");

    let mut reopened = document_at(&path);
    assert_eq!(
        reopened.objects().len(),
        lines.len() - 2,
        "one bad row should cost one object and no more"
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&sidecar);
}

// -- the manipulator's targets ----------------------------------------------

use clayspace_model::{GizmoDrag, GizmoHandle, GizmoMode, GizmoTarget, Transform};

fn drag(mode: GizmoMode, handle: GizmoHandle, pivot: [f32; 3], anchor: [f32; 3]) -> GizmoDrag {
    GizmoDrag {
        mode,
        handle,
        pivot,
        anchor,
        view_axis: [0.0, 0.0, 1.0],
    }
}

#[test]
fn a_drag_moves_a_placed_object() {
    let mut document = document();
    let id = document
        .place_object(Shape::Sphere, &[0.4], [0.0, 0.9, 0.0], subtracting())
        .expect("place");
    let target = GizmoTarget::Object(id);

    let current = document.target_transform(target).expect("a transform");
    assert_eq!(current.position, [0.0, 0.9, 0.0]);

    let gesture = drag(
        GizmoMode::Move,
        GizmoHandle::Axis(1),
        current.position,
        current.position,
    );
    let moved = gesture.resolve(current, [0.0, -0.9, 0.0], false);
    document
        .set_target_transform(target, moved)
        .expect("apply the drag");

    assert!(inside(&document, [0.0, 0.9, 0.0]), "the cavity left");
    assert!(!inside(&document, [0.0, -0.9, 0.0]), "and arrived");
}

#[test]
fn a_drag_moves_a_whole_layer() {
    let mut document = document();
    let key = document.scene().active.expect("an active layer");
    let target = GizmoTarget::Layer(key);

    let current = document.target_transform(target).expect("a transform");
    assert_eq!(
        current.position, [0.0; 3],
        "a layer starts where it was made"
    );

    let gesture = drag(GizmoMode::Move, GizmoHandle::Axis(1), [0.0; 3], [0.0; 3]);
    let moved = gesture.resolve(current, [0.0, 2.0, 0.0], false);
    document
        .set_target_transform(target, moved)
        .expect("move the layer");

    // Everything the layer holds moved with it: the starting form was a unit
    // sphere at the origin and is now two units up.
    assert!(
        !inside(&document, [0.0, 0.0, 0.0]),
        "the origin is empty now"
    );
    assert!(
        inside(&document, [0.0, 2.0, 0.0]),
        "and the form is up there"
    );
}

/// The narrow route and the manipulator address the same value, which is what
/// the spec asks for: a layer moved by dragging reads back as moved.
#[test]
fn both_ways_of_moving_a_layer_agree() {
    let mut document = document();
    let key = document.scene().active.expect("an active layer");
    let target = GizmoTarget::Layer(key);

    document
        .set_layer_transform(key, [0.0, 1.5, 0.0], 1.0)
        .expect("the narrow route");
    let read_back = document.target_transform(target).expect("a transform");
    assert_eq!(read_back.position, [0.0, 1.5, 0.0]);

    document
        .set_target_transform(
            target,
            Transform {
                position: [0.0, -1.5, 0.0],
                ..read_back
            },
        )
        .expect("the manipulator's route");
    let read_back = document.target_transform(target).expect("a transform");
    assert_eq!(read_back.position, [0.0, -1.5, 0.0]);
}

/// Scale is uniform on a layer as on an object, because the engine's
/// transforms take one factor.
#[test]
fn scaling_a_layer_scales_all_of_it() {
    let mut document = document();
    let key = document.scene().active.expect("an active layer");
    let target = GizmoTarget::Layer(key);
    let current = document.target_transform(target).expect("a transform");

    document
        .set_target_transform(
            target,
            Transform {
                scale: 2.0,
                ..current
            },
        )
        .expect("scale");

    // The unit sphere is twice the size, so a point at 1.5 is now inside it.
    assert!(inside(&document, [0.0, 1.5, 0.0]), "the form grew");
}

#[test]
fn a_curve_is_not_transformed_through_an_engine_transform() {
    let mut document = document();
    assert_eq!(
        document.target_transform(GizmoTarget::Curve),
        None,
        "a curve's points belong to the application, not to a node"
    );
    assert!(document
        .set_target_transform(GizmoTarget::Curve, Transform::default())
        .is_err());
}

#[test]
fn a_target_that_is_gone_has_no_transform() {
    let mut document = document();
    let id = document
        .place_object(Shape::Sphere, &[0.3], [0.0, 0.9, 0.0], subtracting())
        .expect("place");
    document.remove_object(id).expect("remove");
    assert_eq!(document.target_transform(GizmoTarget::Object(id)), None);
}

// -- curves -----------------------------------------------------------------

/// A curve turns and scales as a cage does, because it goes through the same
/// arithmetic. Worth a test rather than a comment: the point of routing it
/// that way is that neither has its own implementation to drift from.
#[test]
fn a_curve_turns_about_the_middle_of_its_selection() {
    use clayspace_model::CurveModel;

    let mut document = document();
    document.begin_curve();
    for (at, radius) in [
        ([-0.5f32, 1.4, 0.0], 0.12f32),
        ([0.0, 1.4, 0.0], 0.12),
        ([0.5, 1.4, 0.0], 0.12),
    ] {
        document.add_curve_point(at, radius).expect("point");
    }
    // Adding a point selects it, so only the ones that are not already in the
    // selection are toggled in.
    for index in 0..3 {
        if !document.curve().selection.contains(&index) {
            document.toggle_curve_point(index);
        }
    }
    assert_eq!(document.curve().selection.len(), 3, "all three are picked");

    let pivot = document.curve_pivot().expect("a pivot");
    assert!(
        (pivot[0]).abs() < 1e-4 && (pivot[1] - 1.4).abs() < 1e-4,
        "the pivot should sit in the middle of the three, got {pivot:?}"
    );

    let quarter = drag(
        GizmoMode::Rotate,
        GizmoHandle::Axis(2),
        pivot,
        [0.5, 1.4, 0.0],
    );
    document
        .drag_curve_points(quarter, [0.0, 1.9, 0.0], false)
        .expect("turn");

    let turned = document.curve();
    // A quarter turn about z takes the point that was at +x to +y.
    let last = turned.points.last().expect("a point");
    assert!(
        last.position[1] > 1.7,
        "the end point should have swung up, got {:?}",
        last.position
    );
    // And the middle of them has not moved, because a turn is about it.
    let after = document.curve_pivot().expect("a pivot");
    assert!(
        (after[0] - pivot[0]).abs() < 1e-3 && (after[1] - pivot[1]).abs() < 1e-3,
        "the selection's middle moved: {pivot:?} to {after:?}"
    );
}

#[test]
fn a_curve_with_nothing_selected_has_no_manipulator() {
    use clayspace_model::CurveModel;

    let mut document = document();
    assert_eq!(document.curve_pivot(), None, "no curve, no manipulator");
    document.begin_curve();
    document
        .add_curve_point([0.0, 1.4, 0.0], 0.1)
        .expect("point");
    document.select_curve_point(None);
    assert_eq!(
        document.curve_pivot(),
        None,
        "a curve with nothing picked has nothing to act on"
    );
}

/// The difference between "you cannot transform that" and "you hit nothing".
///
/// A click on a sculpting stroke has to say the first. `pick_object` answers
/// `None` for both, which is why there is a second question.
#[test]
fn a_click_on_a_stroke_says_what_it_hit() {
    use clayspace_model::ItemKind;

    let mut document = document();
    // A lump on top of the form, well clear of the starting sphere's own
    // surface, so a ray straight down attributes to the stroke rather than to
    // the sphere under it.
    document
        .apply_stroke(
            clayspace_model::ToolKind::Padrao,
            clayspace_model::BrushSettings {
                size: 0.3,
                ..clayspace_model::BrushSettings::default()
            },
            &[clayspace_model::GestureSample {
                position: [0.0, 1.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("stroke");

    let down = ([0.0, 4.0, 0.0], [0.0, -1.0, 0.0]);
    let kind = document
        .pick_item(down.0, down.1)
        .expect("something was hit");
    assert!(
        matches!(kind, ItemKind::Stroke | ItemKind::Object),
        "a ray onto the worked form should attribute to something, got {kind:?}"
    );
    // Whatever it attributed to, a stroke is never offered as an object.
    if kind == ItemKind::Stroke {
        assert_eq!(
            document.pick_object(down.0, down.1),
            None,
            "a stroke is not an object"
        );
    }

    // And a ray into empty space hits nothing at all, which is the case the
    // interface must not confuse with the above.
    assert_eq!(document.pick_item([9.0, 9.0, 9.0], [0.0, 1.0, 0.0]), None);
}

/// The manipulator's own rules, on a target that is not a cage.
///
/// The cage holds these already and they are the reason a manipulator feels
/// like one: the widget sits on the middle of what it acts on, an axis handle
/// constrains, a wandering drag lands where it settles, and a scale never
/// passes through zero. A second kind of target that quietly broke one of them
/// would be a manipulator that means two different things.
mod the_manipulators_rules {
    use super::*;

    fn placed() -> (ClayDocument, GizmoTarget, Transform) {
        let mut document = document();
        let id = document
            .place_object(
                Shape::Box,
                &Shape::Box.defaults(),
                [0.0, 0.9, 0.0],
                subtracting(),
            )
            .expect("place");
        let target = GizmoTarget::Object(id);
        let current = document.target_transform(target).expect("a transform");
        (document, target, current)
    }

    #[test]
    fn an_axis_handle_constrains_the_drag() {
        let (mut document, target, current) = placed();
        let gesture = drag(
            GizmoMode::Move,
            GizmoHandle::Axis(1),
            current.position,
            current.position,
        );
        // A hand that drifted sideways while pulling the vertical shaft.
        let moved = gesture.resolve(current, [0.7, 1.6, -0.4], false);
        document.set_target_transform(target, moved).expect("apply");

        let after = document.target_transform(target).expect("a transform");
        assert_eq!(
            [after.position[0], after.position[2]],
            [0.0, 0.0],
            "the drift reached the object: {:?}",
            after.position
        );
        assert!(after.position[1] > current.position[1]);
    }

    #[test]
    fn a_wandering_drag_lands_where_it_ends() {
        let (mut document, target, current) = placed();
        let gesture = drag(
            GizmoMode::Move,
            GizmoHandle::Centre,
            current.position,
            current.position,
        );
        // Resolved from where it began every frame, so the intermediate point
        // leaves no trace.
        for at in [[2.0, 3.0, 1.0], [-1.0, 0.5, 2.0], [0.0, 1.5, 0.0]] {
            let moved = gesture.resolve(current, at, false);
            document.set_target_transform(target, moved).expect("apply");
        }
        let after = document.target_transform(target).expect("a transform");

        let mut fresh = document_fresh();
        let id = fresh
            .place_object(
                Shape::Box,
                &Shape::Box.defaults(),
                [0.0, 0.9, 0.0],
                subtracting(),
            )
            .expect("place");
        let straight = GizmoTarget::Object(id);
        let start = fresh.target_transform(straight).expect("a transform");
        let gesture = drag(
            GizmoMode::Move,
            GizmoHandle::Centre,
            start.position,
            start.position,
        );
        fresh
            .set_target_transform(straight, gesture.resolve(start, [0.0, 1.5, 0.0], false))
            .expect("apply");
        let direct = fresh.target_transform(straight).expect("a transform");

        assert_eq!(
            after.position, direct.position,
            "a drag that wandered should land where a straight one did"
        );
    }

    #[test]
    fn a_scale_never_passes_through_zero() {
        let (mut document, target, current) = placed();
        let gesture = drag(
            GizmoMode::Scale,
            GizmoHandle::Centre,
            [0.0; 3],
            [0.0, 0.9, 0.0],
        );
        // Dragged onto the pivot itself, which asks for a factor of nothing.
        let collapsed = gesture.resolve(current, [0.0; 3], false);
        document
            .set_target_transform(target, collapsed)
            .expect("apply");

        let after = document.target_transform(target).expect("a transform");
        assert!(after.scale > 0.0, "scale reached {}", after.scale);
    }

    #[test]
    fn the_manipulator_offers_no_axis_scale_on_a_transform() {
        assert_eq!(
            GizmoHandle::all_for_transform(GizmoMode::Scale),
            vec![GizmoHandle::Centre],
            "an axis box would measure a stretch the engine cannot apply"
        );
    }
}

/// Thirty frames of dragging is one thing a sculptor did.
#[test]
fn a_whole_drag_is_one_undo_step() {
    let mut document = document();
    let id = document
        .place_object(Shape::Sphere, &[0.4], [0.0, 0.9, 0.0], subtracting())
        .expect("place");
    let target = GizmoTarget::Object(id);
    let start = document.target_transform(target).expect("a transform");
    let gesture = drag(
        GizmoMode::Move,
        GizmoHandle::Axis(1),
        start.position,
        start.position,
    );

    document.begin_target_drag(target);
    for step in 1..=20 {
        let to = [0.0, 0.9 - step as f32 * 0.09, 0.0];
        let moved = gesture.resolve(start, to, false);
        document.set_target_transform(target, moved).expect("frame");
    }
    document.end_target_drag();

    let after = document.target_transform(target).expect("a transform");
    assert!(after.position[1] < 0.0, "it ended up down there");

    document.undo().expect("undo once");
    let back = document.target_transform(target).expect("still there");
    assert!(
        (back.position[1] - 0.9).abs() < 0.2,
        "one undo should take the whole drag back, got {:?}",
        back.position
    );
    assert!(
        !inside(&document, [0.0, 0.9, 0.0]),
        "and the cavity is where the drag started"
    );
}

/// The mirror is a property of the layer that evaluation reads, not an edit
/// baked into the items — so it has to follow an object that moves, not just
/// one that is placed.
#[test]
fn a_moved_object_is_still_mirrored() {
    let mut document = document();
    let id = document
        .place_object(Shape::Sphere, &[0.35], [0.0, 0.9, 0.0], subtracting())
        .expect("place");
    // On the mirror plane to begin with, so there is one cavity.
    assert!(!inside(&document, [0.0, 0.9, 0.0]));

    // Moved off the plane, where the mirror should make a second.
    document
        .set_object_transform(id, [0.7, 0.3, 0.0], [0.0, 1.0, 0.0], 0.0, 1.0)
        .expect("move");

    assert!(
        !inside(&document, [0.7, 0.3, 0.0]),
        "the cavity is where it was moved to"
    );
    assert!(
        !inside(&document, [-0.7, 0.3, 0.0]),
        "and the mirror made its reflection"
    );
}

// -- a custom object as an operand ------------------------------------------

/// A document with a mesh layer, by the route a mesh layer actually takes into
/// one: marched off the field it came from.
fn with_a_mesh() -> (ClayDocument, clayspace_model::LayerKey) {
    let mut document = document();
    let sdf = document.scene().active.expect("a starting layer");
    let mesh = document
        .convert_layer(clayspace_model::Direction::SdfToMesh, 0.05, 1)
        .expect("cross to a mesh");
    // Back to the field, which is where an object can be placed.
    document.set_active_layer(sdf).expect("back to the field");
    (document, mesh)
}

#[test]
fn a_mesh_can_be_placed_as_an_operand() {
    let (mut document, mesh) = with_a_mesh();
    let before = document.objects().len();

    let id = document
        .place_mesh_object(mesh, 0.05, [0.0, 1.4, 0.0], subtracting())
        .expect("place the mesh as an operand");

    let listed = document.objects();
    assert_eq!(listed.len(), before + 1);
    let object = listed.iter().find(|o| o.id == id).expect("listed");
    assert!(
        object.source.shape().is_none(),
        "a mesh operand is not one of the offered shapes"
    );
    assert!(
        !object.label().is_empty(),
        "and it is named after the layer it came from"
    );
}

/// The mesh layer stays exactly as it was: what is placed is a copy, sampled
/// onto a lattice. The mesh is still a mesh and still sculptable.
#[test]
fn placing_a_mesh_operand_leaves_the_mesh_alone() {
    let (mut document, mesh) = with_a_mesh();
    let layers_before = document.scene().layers.len();

    document
        .place_mesh_object(mesh, 0.05, [0.0, 1.4, 0.0], subtracting())
        .expect("place");

    assert_eq!(
        document.scene().layers.len(),
        layers_before,
        "placing an operand should add no layer"
    );
    let still_there = document
        .scene()
        .layers
        .iter()
        .any(|layer| layer.key == mesh && layer.representation == Representation::Mesh);
    assert!(still_there, "the mesh layer went missing");
}

/// The costs stated on use are the panel's own, for the same crossing at the
/// same resolution — because it is the same crossing.
#[test]
fn the_stated_cost_is_the_conversions_own() {
    let (mut document, mesh) = with_a_mesh();
    let cost = document
        .mesh_operand_cost(mesh, 0.05)
        .expect("a mesh has a crossing cost");

    assert!(
        cost.surface_movement > 0.0,
        "a crossing moves the surface and should say so"
    );
    assert!(
        (cost.vanishing_feature - 0.05).abs() < 1e-6,
        "a feature thinner than a cell is lost, so it is the cell: {}",
        cost.vanishing_feature
    );
    assert!(!cost.keeps_sharp_edges, "sharp edges go to a staircase");

    // A finer cell moves the surface less, which is the whole reason the
    // resolution is a choice.
    let finer = document.mesh_operand_cost(mesh, 0.01).expect("a cost");
    assert!(finer.surface_movement < cost.surface_movement);
}

#[test]
fn a_layer_that_is_not_a_mesh_is_not_offered_and_is_refused() {
    let mut document = document();
    let sdf = document.scene().active.expect("a layer");

    assert!(
        !document.mesh_operands().iter().any(|(key, _)| *key == sdf),
        "a field is not a mesh operand"
    );
    assert!(document.mesh_operand_cost(sdf, 0.05).is_none());
    assert!(document
        .place_mesh_object(sdf, 0.05, [0.0; 3], subtracting())
        .is_err());
}

#[test]
fn the_mesh_operands_are_the_mesh_layers() {
    let (mut document, mesh) = with_a_mesh();
    let offered = document.mesh_operands();
    assert!(offered.iter().any(|(key, _)| *key == mesh));
    assert!(
        offered.iter().all(|(_, name)| !name.is_empty()),
        "an operand with no name is a row a sculptor cannot read"
    );
}

#[test]
fn a_mesh_operand_survives_a_reopen() {
    let (mut document, mesh) = with_a_mesh();
    let path = std::env::temp_dir().join("clayspace-mesh-operand.clay");
    let _ = std::fs::remove_file(&path);
    let id = document
        .place_mesh_object(mesh, 0.05, [0.0, 1.4, 0.0], subtracting())
        .expect("place");
    document.save(&path).expect("save");

    let mut reopened = document_at(&path);
    let listed = reopened.objects();
    let object = listed
        .iter()
        .find(|object| object.id == id)
        .expect("the operand came back");
    assert!(object.source.shape().is_none(), "still a mesh operand");
    assert_eq!(object.combine.op, Combine::Subtract);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(clayspace_engine::objects::sidecar_for(&path));
}
