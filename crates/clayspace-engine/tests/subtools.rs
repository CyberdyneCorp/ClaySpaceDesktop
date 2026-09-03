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

/// The box the viewport's own triangles fall in, or nothing where a document
/// draws none. What is *drawn*, as against what a layer says it holds.
fn drawn_bounds(doc: &mut ClayDocument) -> Option<([f32; 3], [f32; 3])> {
    let (positions, _, _, _, _) = doc.visible_mesh_geometry();
    let first = *positions.first()?;
    let mut min = first;
    let mut max = first;
    for point in &positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    Some((min, max))
}

/// Where the surface stands along a ray, or nothing.
fn surface_at(doc: &ClayDocument, x: f32) -> Option<[f32; 3]> {
    let (origin, direction) = ray_at(x);
    doc.pick(origin, direction)
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

/// A stroke on a moved subtool lands where the sculptor touched it.
///
/// The regression: a field layer's transform moves what the tape evaluates, so
/// the form is drawn and picked where the manipulator put it — but the stroke
/// went to the engine in world coordinates and the stamps were deposited in
/// the layer's own frame, which the transform then moved *again*. A subtool
/// dragged three units along X was sculpted three units past the pointer, and
/// the surface under the brush never moved.
#[test]
fn a_stroke_lands_where_a_moved_subtool_is_drawn() {
    let (mut doc, _first, second) = two_subtools();
    doc.set_active_layer(second).expect("sculpt the moved one");

    let before = surface_at(&doc, APART).expect("the moved form's near face");
    dab(&mut doc, before).expect("a dab on the face the pointer found");
    let after = surface_at(&doc, APART).expect("the moved form is still there");

    // Padrão raises the surface along its normal, so the near face comes
    // toward the eye. Anything else means the stamp went somewhere the
    // sculptor was not pointing.
    assert!(
        after[2] < before[2] - 0.01,
        "a dab on a subtool standing at x = {APART} left its face at {after:?}, \
         where it was {before:?}: the stroke did not land under the pointer"
    );
}

/// The mirror follows the subtool it is mirroring.
///
/// Symmetry is reflected in the layer's own frame — which is what the engine's
/// layer mirror does to the items a stamping stroke deposits — so a dab to one
/// side of a moved subtool's axis is answered on the other side of *that* axis
/// and not on the other side of the world's.
#[test]
fn a_mirrored_stroke_answers_across_the_moved_subtools_own_axis() {
    let (mut doc, _first, second) = two_subtools();
    doc.set_active_layer(second).expect("sculpt the moved one");

    // Off the subtool's axis by a third of its radius, on the near face.
    const OFF: f32 = 0.2;
    let left = surface_at(&doc, APART - OFF).expect("the near face, left of the axis");
    let right_before = surface_at(&doc, APART + OFF).expect("and right of it");

    let samples = [GestureSample {
        position: left,
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
        [true, false, false],
    )
    .expect("a mirrored dab");

    let right_after = surface_at(&doc, APART + OFF).expect("the reflection's side is still there");
    assert!(
        right_after[2] < right_before[2] - 0.01,
        "a dab {OFF} left of a subtool standing at x = {APART} left the face \
         {OFF} to its right at {right_after:?}, where it was {right_before:?}: \
         the mirror is not reflecting across the subtool's own axis"
    );
}

/// A mask painted on a moved subtool protects what is under the brush.
///
/// The same root as the stroke above, at the other end of it: a mask belongs
/// to its layer and every consumer reads it where the layer's own content is —
/// the gate on a stamp, the engine's stroke mask, the mesh sculptor — while
/// the brush painted it where the form is *drawn*. On a moved subtool those
/// are two places, so the frozen region sat beside the form it was meant to
/// protect and the freeze quietly did nothing.
#[test]
fn a_mask_painted_on_a_moved_subtool_protects_what_is_under_the_brush() {
    // What the dab does with nothing in its way, to measure the freeze
    // against: a threshold on its own would pass on a stroke that had stopped
    // working for some other reason.
    let (mut doc, _first, second) = two_subtools();
    doc.set_active_layer(second).expect("work the moved one");
    let free_before = surface_at(&doc, APART).expect("the near face");
    dab(&mut doc, free_before).expect("an unmasked dab");
    let free = free_before[2] - surface_at(&doc, APART).expect("face")[2];
    assert!(
        free > 0.1,
        "the unmasked dab moved the surface by {free}; there is nothing to \
         measure a freeze against"
    );

    let (mut doc, _first, second) = two_subtools();
    doc.set_active_layer(second).expect("work the moved one");
    let hit = surface_at(&doc, APART).expect("the near face");
    let samples = [GestureSample {
        position: hit,
        pressure: 1.0,
        time: 0.0,
    }];
    doc.apply_stroke(
        ToolKind::Mascara,
        BrushSettings {
            size: 0.6,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &samples,
        [false; 3],
    )
    .expect("freeze the face under the pointer");

    // It reads back where the sculptor painted it, which is what the viewport
    // asks in order to draw the frozen region.
    let frozen = doc.mask_at(&[hit]).expect("a mask to read")[0];
    assert!(
        frozen > 0.9,
        "the mask reads {frozen} where it was painted; the frozen region is \
         not where the brush was"
    );

    dab(&mut doc, hit).expect("a dab on the frozen face");
    let moved = hit[2] - surface_at(&doc, APART).expect("face")[2];
    assert!(
        moved < free * 0.2,
        "a dab on a frozen face of a subtool standing at x = {APART} moved the \
         surface by {moved}, against {free} unfrozen: the mask did not protect it"
    );
}

/// The same invariant on the other two representations: a dab lands where the
/// pointer found the surface, whatever a layer transform does or does not
/// reach.
///
/// The three do not agree about that, which is why this asks the pointer
/// rather than assuming. A field layer's tape is moved by the engine and a
/// carried mesh's vertices are moved by the host, so both stand where the
/// manipulator put them. A grid is moved by neither — ClayCore cannot place
/// one — so a voxel subtool stays where its cells are while the widget moves
/// off it. In every case the surface a ray meets is the surface a dab must
/// land on, and that is what is measured here.
#[test]
fn a_dab_lands_under_the_pointer_on_a_moved_mesh_and_on_a_moved_grid() {
    // A carried mesh, moved: the pointer finds it where it was put.
    let mut doc = document();
    let key = doc
        .convert_layer(clayspace_model::Direction::SdfToMesh, 0.08, 0)
        .expect("a mesh subtool");
    doc.set_active_layer(key).expect("work the mesh");
    doc.set_layer_transform(key, [APART, 0.0, 0.0], 1.0)
        .expect("move it");
    let (origin, direction) = ray_at(APART);
    let before = doc
        .pick(origin, direction)
        .expect("a moved mesh stands where the manipulator put it");
    dab(&mut doc, before).expect("a dab on the face the pointer found");
    let after = doc.pick(origin, direction).expect("still there");
    assert!(
        after[2] < before[2] - 0.005,
        "a dab on a mesh subtool standing at x = {APART} left its face at \
         {after:?}, where it was {before:?}"
    );

    // A grid, moved the same way: the host places its cells, so the pointer
    // finds it where the manipulator put it and the dab lands there.
    let mut doc = document();
    doc.add_voxel_layer("Grade", 0.04).expect("a grid");
    let key = doc.scene().active.expect("the grid is active");
    dab(&mut doc, [0.0, 0.0, 0.0]).expect("something to sculpt");
    doc.set_layer_transform(key, [APART, 0.0, 0.0], 1.0)
        .expect("move it");
    let (origin, direction) = ray_at(APART);
    let before = doc
        .pick(origin, direction)
        .expect("a moved grid stands where the manipulator put it");
    dab(&mut doc, before).expect("a dab on the face the pointer found");
    let after = doc.pick(origin, direction).expect("still there");
    assert!(
        after[2] < before[2] - 0.005,
        "a dab on a grid standing at x = {APART} left its face at {after:?}, \
         where it was {before:?}"
    );
}

/// A grid moves with its subtool: drawn, framed and picked where the
/// manipulator put it.
///
/// The regression: ClayCore holds a voxel layer's placement and composes it
/// wherever the *document* answers — `clay_layer_bounds` reports the moved box
/// — but every voxel entry point is in the grid's own coordinates, so placing
/// one is the host's to do, exactly as it is for a carried mesh. It was not
/// done: the widget moved and the form stayed, and Frame All framed where the
/// sculpt had been.
#[test]
fn a_moved_grid_is_drawn_framed_and_picked_where_it_was_put() {
    let mut doc = document();
    doc.add_voxel_layer("Grade", 0.04).expect("a grid");
    let key = doc.scene().active.expect("the grid is active");
    dab(&mut doc, [0.0, 0.0, 0.0]).expect("something to look at");

    let drawn_before = drawn_bounds(&mut doc).expect("a grid draws triangles");
    let framed_before = doc.layer_bounds(key).expect("and reports a box");
    assert!(
        doc.pick(ray_at(0.0).0, ray_at(0.0).1).is_some(),
        "the grid should be pickable where it was built"
    );

    doc.set_layer_transform(key, [APART, 0.0, 0.0], 1.0)
        .expect("move it");

    let drawn = drawn_bounds(&mut doc).expect("a moved grid still draws");
    assert!(
        (drawn.0[0] - drawn_before.0[0] - APART).abs() < 0.05,
        "a grid moved to x = {APART} draws at {drawn:?}, where it drew \
         {drawn_before:?}: the placement did not reach what the viewport shows"
    );
    let framed = doc.layer_bounds(key).expect("and reports a box");
    assert!(
        (framed.0[0] - framed_before.0[0] - APART).abs() < 0.05,
        "a grid moved to x = {APART} reports {framed:?}, where it reported \
         {framed_before:?}: the manipulator and Frame All read this box"
    );
    assert!(
        doc.pick(ray_at(APART).0, ray_at(APART).1).is_some(),
        "a moved grid is not pickable where it is drawn"
    );
    assert!(
        doc.pick(ray_at(0.0).0, ray_at(0.0).1).is_none(),
        "a moved grid is still pickable where it no longer is"
    );
}

/// And a mask on one freezes the cells under the brush, wherever it stands.
#[test]
fn a_mask_on_a_moved_grid_protects_what_is_under_the_brush() {
    let mut doc = document();
    doc.add_voxel_layer("Grade", 0.04).expect("a grid");
    let key = doc.scene().active.expect("the grid is active");
    dab(&mut doc, [0.0, 0.0, 0.0]).expect("something to freeze");
    doc.set_layer_transform(key, [APART, 0.0, 0.0], 1.0)
        .expect("move it");

    let (origin, direction) = ray_at(APART);
    let hit = doc
        .pick(origin, direction)
        .expect("the moved grid's near face");
    doc.apply_stroke(
        ToolKind::Mascara,
        BrushSettings {
            size: 0.5,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: hit,
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    )
    .expect("freeze the face under the pointer");

    let frozen = doc.mask_at(&[hit]).expect("a mask to read")[0];
    assert!(
        frozen > 0.9,
        "the mask on a moved grid reads {frozen} where it was painted"
    );
}
