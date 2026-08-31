//! Working a scene that holds more than one form.
//!
//! The engine and the model have been multi-layer from the start; what was
//! missing was the consequence. Clicking a form did not make it the thing a
//! brush lands on, and the manipulator that moves a whole layer was
//! implemented, tested at the engine boundary, and unreachable from any
//! control. These hold the two halves of that: a pick resolves to a layer and
//! activation follows it, and a drag on a whole subtool moves, turns and
//! scales all of it as one undo step.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, GestureSample, GizmoDrag, GizmoHandle, GizmoMode,
    GizmoTarget, LayerKey, ModelError, ObjectModel, Protection, Representation, SceneModel,
    SculptModel, Shape, ToolKind, Transform, Unavailable,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far along X the second subtool stands.
///
/// Clear of the starting form, which is a unit sphere at the origin, so a ray
/// down one of them cannot reach the other and a test that claims to have
/// picked the second one has.
const APART: f32 = 3.0;

/// Two subtools standing apart: the starting form, and a sphere of its own in
/// a second layer moved clear of it.
///
/// The second layer is placed by its *layer* transform rather than by putting
/// the sphere off-centre inside it, because that is what a whole-subtool
/// manipulator addresses — and it leaves the layer's middle where the layer
/// says it is.
fn two_subtools() -> (ClayDocument, LayerKey, LayerKey) {
    let mut doc = document();
    let first = doc.scene().active.expect("a starting layer");
    let second = doc
        .add_layer("Segunda", Representation::Sdf)
        .expect("a second layer");
    doc.place_object(
        Shape::Sphere,
        &[0.6],
        [0.0; 3],
        CombineSettings {
            op: Combine::Add,
            ..CombineSettings::default()
        },
    )
    .expect("a form in the second layer");
    doc.set_layer_transform(second, [APART, 0.0, 0.0], 1.0)
        .expect("stand it clear of the first");
    // Back to the first, which is where a sculptor would be before clicking
    // the second one: the test is about what the click does.
    doc.set_active_layer(first).expect("activate the first");
    (doc, first, second)
}

/// A ray from in front of a point, looking down Z.
fn ray_at(x: f32) -> ([f32; 3], [f32; 3]) {
    ([x, 0.0, -8.0], [0.0, 0.0, 1.0])
}

/// How many items a layer holds.
///
/// A field stroke deposits stamps as items, so this is what says *which* layer
/// a dab landed on — the surface alone cannot, since two layers compose into
/// one.
fn items(doc: &ClayDocument, key: LayerKey) -> usize {
    let id = doc.layer_id(key).expect("a layer");
    doc.document().layer_nodes(id).expect("its nodes").len()
}

/// One dab where a ray meets the surface.
fn dab(doc: &mut ClayDocument, at: [f32; 3]) -> Result<(), clayspace_model::ModelError> {
    let samples = [GestureSample {
        position: at,
        pressure: 1.0,
        time: 0.0,
    }];
    doc.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size: 0.3,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &samples,
        [false; 3],
    )
    .map(|_| ())
}

fn protection(ghost: bool, locked: bool) -> Protection {
    Protection { ghost, locked }
}

// -- activation --------------------------------------------------------------

#[test]
fn a_click_on_a_second_subtool_takes_the_next_dab() {
    let (mut doc, first, second) = two_subtools();
    let (origin, direction) = ray_at(APART);

    let hit = doc.layer_at(origin, direction).expect("the second subtool");
    assert_eq!(
        hit, second,
        "the ray down the second form named another layer"
    );

    doc.set_active_layer(hit).expect("activate what was picked");

    let (before_first, before_second) = (items(&doc, first), items(&doc, second));
    let at = doc.pick(origin, direction).expect("its surface");
    dab(&mut doc, at).expect("a dab on the subtool that was clicked");

    assert!(
        items(&doc, second) > before_second,
        "the dab did not land on the subtool that was clicked"
    );
    assert_eq!(
        items(&doc, first),
        before_first,
        "the subtool that was active before the click was edited anyway"
    );
}

/// The other half of the same rule: picking is what decides, so a click on the
/// *first* form leaves the second one alone.
#[test]
fn a_click_names_the_form_it_landed_on() {
    let (mut doc, first, second) = two_subtools();
    assert_eq!(doc.layer_at(ray_at(0.0).0, ray_at(0.0).1), Some(first));
    assert_eq!(doc.layer_at(ray_at(APART).0, ray_at(APART).1), Some(second));
    assert_eq!(
        doc.layer_at([0.0, 6.0, -8.0], [0.0, 0.0, 1.0]),
        None,
        "a ray over the top of both forms named a layer anyway"
    );
}

/// A ghosted layer is visible and not pickable, so the pick answers with what
/// stands behind it. The engine excludes ghosts from the attributed raycast;
/// this holds that the application honours that answer rather than reaching
/// past it.
#[test]
fn a_ghosted_subtool_passes_the_activation_through() {
    let (mut doc, first, second) = two_subtools();
    // In front of the first form and in line with it, so the two overlap along
    // the ray and the nearer one is the ghost.
    doc.set_layer_transform(second, [0.0, 0.0, -2.0], 1.0)
        .expect("stand the second in front of the first");
    let (origin, direction) = ray_at(0.0);
    assert_eq!(
        doc.layer_at(origin, direction),
        Some(second),
        "the nearer form should be picked while it is ordinary"
    );

    doc.set_layer_protection(second, protection(true, false))
        .expect("ghost it");

    assert_eq!(
        doc.layer_at(origin, direction),
        Some(first),
        "the ghost took the pick; a ghosted subtool must pass it through to \
         what stands behind it"
    );
}

/// Locked is not ghosted: the spec says a locked layer is still pickable, so
/// it activates. What it refuses is the edit, by name.
#[test]
fn a_locked_subtool_activates_and_refuses_the_dab_with_its_reason() {
    let (mut doc, _first, second) = two_subtools();
    doc.set_layer_protection(second, protection(false, true))
        .expect("lock it");

    let (origin, direction) = ray_at(APART);
    let hit = doc
        .layer_at(origin, direction)
        .expect("a locked subtool is still pickable");
    assert_eq!(hit, second);
    doc.set_active_layer(hit).expect("activate it");
    assert_eq!(doc.scene().active, Some(second));

    let at = doc.pick(origin, direction).expect("its surface");
    let error = dab(&mut doc, at).expect_err("a locked subtool accepts no edit");
    assert!(
        matches!(error, ModelError::Unavailable(Unavailable::LayerProtected)),
        "the refusal did not name locking as the cause: {error}"
    );
    // And the cause has a sentence the interface can show, which is what makes
    // it a refusal rather than a silent no.
    assert_eq!(
        doc.scene()
            .layer(second)
            .and_then(|layer| layer.protection.refusal()),
        Some("esta camada está bloqueada"),
    );
}

/// The whole point of collapsing the two fields: there is no picked layer
/// beside the sculpted one that could disagree with it.
#[test]
fn the_scene_reports_one_active_layer() {
    let (mut doc, first, second) = two_subtools();
    assert_eq!(doc.scene().active, Some(first));
    doc.set_active_layer(second).expect("activate the second");
    assert_eq!(doc.scene().active, Some(second));
    // A pick answers; it does not activate. The one mutation is
    // `set_active_layer`, which `Command::SelectLayer` is the only route to.
    let (origin, direction) = ray_at(0.0);
    assert_eq!(doc.layer_at(origin, direction), Some(first));
    assert_eq!(
        doc.scene().active,
        Some(second),
        "the pick moved the sculpt target on its own"
    );
}

// -- the whole-subtool manipulator -------------------------------------------

/// Whether the surface encloses a point.
fn inside(doc: &ClayDocument, at: [f32; 3]) -> bool {
    doc.document()
        .eval_points(None, &[at])
        .is_ok_and(|values| values[0] < 0.0)
}

/// One manipulator gesture on a target, grouped as a drag is.
///
/// `begin_target_drag`/`end_target_drag` are what make a drag one undo step
/// however many frames it took, which is the half of this the interface owes.
/// The anchor is where the hand took hold: a turn measures the angle between
/// it and where the hand went, so a gesture anchored *on* the pivot has no
/// angle to report and would pass a test that did nothing.
fn manipulate(
    doc: &mut ClayDocument,
    target: GizmoTarget,
    mode: GizmoMode,
    handle: GizmoHandle,
    anchor: [f32; 3],
    to: [f32; 3],
) -> Transform {
    let at = doc.target_transform(target).expect("a transform");
    let gesture = GizmoDrag {
        mode,
        handle,
        pivot: at.position,
        anchor,
        view_axis: [0.0, 0.0, 1.0],
    };
    doc.begin_target_drag(target);
    doc.set_target_transform(target, gesture.resolve(at, to, false))
        .expect("apply the drag");
    doc.end_target_drag();
    at
}

#[test]
fn dragging_the_manipulator_moves_a_whole_subtool() {
    let (mut doc, _first, second) = two_subtools();
    let target = GizmoTarget::Layer(second);

    assert!(inside(&doc, [APART, 0.0, 0.0]), "the subtool starts there");
    manipulate(
        &mut doc,
        target,
        GizmoMode::Move,
        GizmoHandle::Centre,
        [APART, 0.0, 0.0],
        [APART, 2.0, 0.0],
    );

    assert!(
        !inside(&doc, [APART, 0.0, 0.0]),
        "the subtool did not leave where it was"
    );
    assert!(
        inside(&doc, [APART, 2.0, 0.0]),
        "the subtool did not arrive where it was dragged"
    );
}

#[test]
fn dragging_the_manipulator_turns_a_whole_subtool() {
    let (mut doc, _first, second) = two_subtools();
    let target = GizmoTarget::Layer(second);
    // A sphere is its own rotation, so a turn is read from the transform the
    // engine holds rather than from the silhouette. What this holds is that a
    // turn reaches a *layer* at all — the manipulator's third mode was as
    // unreachable as the other two.
    let before = doc.target_transform(target).expect("a transform");
    assert_eq!(before.rotation_angle, 0.0);

    // A quarter turn about Z: taken hold of on the ring beside the form and
    // carried a quarter of the way round it.
    manipulate(
        &mut doc,
        target,
        GizmoMode::Rotate,
        GizmoHandle::Axis(2),
        [APART + 1.0, 0.0, 0.0],
        [APART, 1.0, 0.0],
    );

    let after = doc.target_transform(target).expect("a transform");
    assert!(
        (after.rotation_angle.abs() - std::f32::consts::FRAC_PI_2).abs() < 1e-3,
        "the turn did not reach the layer: {} rather than a quarter turn",
        after.rotation_angle
    );
}

#[test]
fn dragging_the_manipulator_scales_a_whole_subtool() {
    let (mut doc, _first, second) = two_subtools();
    let target = GizmoTarget::Layer(second);
    // The sphere has a radius of 0.6, so this point is outside it and inside
    // the same sphere at twice the size.
    let outside = [APART + 0.9, 0.0, 0.0];
    assert!(!inside(&doc, outside));

    // Taken hold of one unit out and carried to two: a doubling, uniform,
    // which is the only scale an engine transform takes.
    manipulate(
        &mut doc,
        target,
        GizmoMode::Scale,
        GizmoHandle::Centre,
        [APART + 1.0, 0.0, 0.0],
        [APART + 2.0, 0.0, 0.0],
    );

    // A layer scales uniformly — the engine's layer transform takes one
    // factor, unlike a node's — so the three components stay equal and the
    // one number is exact.
    assert_eq!(
        doc.target_transform(target).map(|at| at.scale),
        Some([2.0; 3]),
        "the drag asked for a doubling and the layer did not take it"
    );
    assert!(
        inside(&doc, outside),
        "the subtool did not grow; a uniform scale on a layer went nowhere"
    );
}

/// The gesture is grouped, so taking it back is one undo and not one per
/// frame the pointer moved.
#[test]
fn one_undo_reverts_a_whole_subtool_drag() {
    let (mut doc, _first, second) = two_subtools();
    let target = GizmoTarget::Layer(second);

    let before = doc.target_transform(target).expect("a transform");
    let at = before;
    let gesture = GizmoDrag {
        mode: GizmoMode::Move,
        handle: GizmoHandle::Centre,
        pivot: at.position,
        anchor: at.position,
        view_axis: [0.0, 0.0, 1.0],
    };
    doc.begin_target_drag(target);
    // Several frames of one drag, which is what a hand produces.
    for step in 1..=4 {
        let to = [APART, step as f32 * 0.5, 0.0];
        doc.set_target_transform(target, gesture.resolve(at, to, false))
            .expect("a frame of the drag");
    }
    doc.end_target_drag();
    assert!(
        inside(&doc, [APART, 2.0, 0.0]),
        "the drag moved the subtool"
    );

    assert!(doc.undo().expect("undo"), "there was nothing to undo");
    assert!(
        inside(&doc, [APART, 0.0, 0.0]),
        "one undo did not bring the whole drag back"
    );
    assert_eq!(
        doc.target_transform(target).map(|at| at.position),
        Some(before.position),
        "the layer transform did not come back with the surface"
    );
}

// -- a subtool that leaves --------------------------------------------------

/// Removing a subtool above the active one leaves the sculpt target where it
/// was.
///
/// `remove_layer` clamped the active *index* rather than following the layer
/// it pointed at, so removing a row above the active one shifted every later
/// row down by one and left `active` where it sat. That was tolerable while
/// the only thing keyed to it was the id; the subtools work makes `active` the
/// selector for the mask, the mirror and the rig as well, so the sculptor's
/// next dab, frozen region, symmetry toggle and armature all silently moved to
/// a subtool nothing said anything about.
#[test]
fn removing_a_subtool_above_the_active_one_keeps_the_sculpt_target() {
    let (mut doc, first, second) = two_subtools();
    let third = doc
        .add_layer("Terceira", Representation::Sdf)
        .expect("a third layer");
    doc.set_active_layer(second).expect("work the second");

    doc.remove_layer(first).expect("remove the first");

    assert_eq!(
        doc.scene().active,
        Some(second),
        "the sculptor removed a different subtool; this one is still the one \
         being worked, and the mask, the mirror and the rig go with it"
    );
    assert_ne!(doc.scene().active, Some(third));
}
