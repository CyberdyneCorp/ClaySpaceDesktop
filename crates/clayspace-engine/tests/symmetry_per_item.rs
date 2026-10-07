//! Symmetry mirrors what is made while it is on, not what was already there.
//!
//! The engine's layer mirror reflects every item that inherits it, past and
//! future. Read naively, that makes the symmetry switch reach backwards — a
//! lump sculpted one-sided grew a twin the moment the next stroke wrote the
//! mirror, a box placed with symmetry off became two when it was turned on,
//! and a lump sculpted under X lost its twin when symmetry was turned off or
//! moved to Z (#170, Y2).
//!
//! Each item carries the axes it was made under (`clay_item_set_mirror_axes`,
//! ClayCore v0.126.0), and the engine reflects it through those whatever the
//! layer's mirror is pointed at. The layer's mirror reaches only the items
//! that inherit it — the starting form, and documents saved before the engine
//! had per-item axes — and only a Move or a Pinçar writes it, since their
//! drag images follow it for those items.
//!
//! And a mirror change moves surface the brick cache holds. The images the old
//! mirror made are gone from the field, and unless the bricks under them are
//! refilled the viewport keeps drawing them — on a hidden layer too, because
//! hiding refills only what the layer reaches *now* (#170, Y1).

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    ArmatureModel, BrushSettings, Combine, CombineSettings, CurveModel, GestureSample, GizmoTarget,
    NodeIndex, ObjectModel, Representation, SceneModel, SculptModel, Shape, ToolKind, Transform,
};

const OFF: [bool; 3] = [false; 3];
const X: [bool; 3] = [true, false, false];
const Z: [bool; 3] = [false, false, true];

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// A short stroke that adds clay at `at`, with the symmetry the sculptor
/// asked for — an Add, so it deposits even clear of any surface.
fn dab(document: &mut ClayDocument, at: [f32; 3], symmetry: [bool; 3]) {
    SculptModel::set_symmetry(document, symmetry).expect("record the setting");
    SculptModel::set_combine(
        document,
        CombineSettings {
            op: Combine::Add,
            ..CombineSettings::default()
        },
    );
    let samples: Vec<GestureSample> = (0..3)
        .map(|i| GestureSample {
            position: [at[0], at[1] + i as f32 * 0.02, at[2]],
            pressure: 1.0,
            time: i as f32,
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.3,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            symmetry,
        )
        .expect("a dab");
}

/// Whether the document's field is solid at `at` — the truth.
fn solid(document: &ClayDocument, at: [f32; 3]) -> bool {
    document
        .document()
        .eval_points(None, &[at])
        .expect("the field answers")[0]
        < 0.0
}

/// Where the brick cache — what the viewport draws — finds the surface
/// straight down through `x`.
fn drawn_top(document: &ClayDocument, x: f32) -> Option<f32> {
    document
        .cache()
        .raycast([x, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("the cache was asked")
        .map(|hit| hit.position[2])
}

/// The same question put to the document.
fn true_top(document: &ClayDocument, x: f32) -> Option<f32> {
    document
        .document()
        .raycast([x, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("the document was asked")
        .map(|hit| hit.position[2])
}

fn assert_drawn_as_it_is(document: &ClayDocument, x: f32, what: &str) {
    let (drawn, truth) = (drawn_top(document, x), true_top(document, x));
    match (drawn, truth) {
        (None, None) => {}
        (Some(drawn), Some(truth)) if (drawn - truth).abs() < 0.03 => {}
        _ => panic!(
            "{what}: at x = {x} the viewport draws a surface at {drawn:?} and \
             the document has it at {truth:?} — a stale brick survived"
        ),
    }
}

/// A short Move drag from `from` by `delta`, with the symmetry the sculptor
/// asked for. Move is one of the two verbs that still write the layer's
/// mirror, so this is also how a test changes it.
fn drag(document: &mut ClayDocument, from: [f32; 3], delta: [f32; 3], symmetry: [bool; 3]) {
    SculptModel::set_symmetry(document, symmetry).expect("record the setting");
    let samples: Vec<GestureSample> = (0..=4)
        .map(|i| {
            let t = i as f32 / 4.0;
            GestureSample {
                position: std::array::from_fn(|axis| from[axis] + delta[axis] * t),
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                size: 0.35,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            symmetry,
        )
        .expect("a drag");
}

/// Stands the starting form at `at`. It inherits its layer's mirror, as the
/// base form of a subtool does, so off the plane it is the one item a later
/// change of the layer's mirror still moves.
fn move_the_starting_form_to(document: &mut ClayDocument, at: [f32; 3]) {
    let id = document.objects()[0].id;
    document
        .set_target_transform(GizmoTarget::Object(id), Transform::at(at))
        .expect("move the starting form");
}

/// A lump sculpted one-sided stays one-sided when symmetry is turned on and
/// the next stroke is made under it.
#[test]
fn turning_symmetry_on_leaves_existing_items_alone() {
    let mut document = document();
    // Well clear of the starting sphere, so the lump is the only thing there.
    let lump = [1.8, 0.0, 0.0];
    let twin = [-1.8, 0.0, 0.0];
    dab(&mut document, lump, OFF);
    assert!(solid(&document, lump), "the dab deposited nothing");
    assert!(!solid(&document, twin), "an unmirrored dab has a twin");

    // Symmetry on: a stroke on the plane, and a drag, which is what writes
    // the layer's mirror.
    dab(&mut document, [0.0, 0.0, -1.0], X);
    drag(&mut document, [0.6, 0.0, 0.8], [0.0, 0.0, 0.2], X);

    assert!(
        !solid(&document, twin),
        "turning symmetry on grew a twin of a lump sculpted while it was off"
    );
    assert!(solid(&document, lump), "the lump itself went");
    assert_drawn_as_it_is(&document, twin[0], "after turning symmetry on");
}

/// What is made while symmetry is on is still mirrored.
#[test]
fn a_stroke_made_with_symmetry_on_is_mirrored() {
    let mut document = document();
    dab(&mut document, [0.0, 0.0, -1.0], OFF);
    dab(&mut document, [1.8, 0.0, 0.0], X);
    assert!(
        solid(&document, [-1.8, 0.0, 0.0]),
        "a dab made with symmetry on has no twin"
    );
    assert_drawn_as_it_is(&document, -1.8, "a mirrored dab");
}

/// A mirror change takes surface away as well as adding it, and the bricks
/// under what it took away are refilled.
///
/// The item that moves is the starting form, stood off the plane: it inherits
/// the layer's mirror, and a drag made with Z symmetry is what re-points that
/// mirror. An item made under symmetry carries its own axes and is not what a
/// layer-mirror change moves any more.
#[test]
fn a_mirror_change_dirties_both_images() {
    let mut document = document();
    move_the_starting_form_to(&mut document, [1.8, 0.0, 0.0]);
    assert!(solid(&document, [-1.8, 0.0, 0.0]), "no twin to take away");
    assert_drawn_as_it_is(&document, -1.8, "before the change");

    // A drag with Z symmetry writes the layer's mirror across z, which takes
    // the inherited twin across x out of the field.
    drag(&mut document, [1.8, 1.0, 0.0], [0.0, 0.2, 0.0], Z);
    assert!(
        !solid(&document, [-1.8, 0.0, 0.0]),
        "the mirror did not change"
    );

    assert_drawn_as_it_is(&document, -1.8, "after the mirror changed");
    assert_drawn_as_it_is(&document, 1.8, "the drag's own side");
}

/// Hidden after a mirror change, a layer leaves nothing behind.
#[test]
fn a_hidden_layer_draws_nothing_after_a_mirror_change() {
    let mut document = document();
    let key = document
        .add_layer("Lado", Representation::Sdf)
        .expect("a second subtool");
    dab(&mut document, [1.8, 0.0, 0.0], X);
    assert!(solid(&document, [-1.8, 0.0, 0.0]), "the lump has no twin");
    drag(&mut document, [1.8, 0.0, 0.3], [0.0, 0.0, 0.1], Z);

    document
        .set_layer_visible(key, false)
        .expect("hide the subtool");
    for x in [-1.8, 1.8] {
        assert_eq!(
            drawn_top(&document, x),
            None,
            "a hidden subtool still draws a surface at x = {x}"
        );
    }
}

/// A curve begun with symmetry on is mirrored, like a stroke.
#[test]
fn a_curve_begun_with_symmetry_on_is_mirrored() {
    let mut document = document();
    // Written off by a stroke first, so the curve has to write it back.
    dab(&mut document, [0.0, 0.0, -1.0], OFF);
    SculptModel::set_symmetry(&mut document, X).expect("symmetry on");

    document.begin_curve();
    for at in [[1.5f32, 0.0, 0.3], [1.8, 0.0, 0.0], [2.1, 0.0, -0.3]] {
        document.add_curve_point(at, 0.12).expect("a point");
    }
    assert!(
        solid(&document, [1.8, 0.0, 0.0]),
        "the curve placed nothing"
    );
    assert!(
        solid(&document, [-1.8, 0.0, 0.0]),
        "a curve begun with symmetry on was not mirrored"
    );
    assert_drawn_as_it_is(&document, -1.8, "the mirrored curve");
}

/// A curve begun with symmetry off is not mirrored by a later change.
#[test]
fn a_curve_made_one_sided_stays_one_sided() {
    let mut document = document();
    SculptModel::set_symmetry(&mut document, OFF).expect("symmetry off");
    document.begin_curve();
    for at in [[1.5f32, 0.0, 0.3], [1.8, 0.0, 0.0], [2.1, 0.0, -0.3]] {
        document.add_curve_point(at, 0.12).expect("a point");
    }
    document.cancel_curve();
    dab(&mut document, [0.0, 0.0, -1.0], X);
    assert!(
        !solid(&document, [-1.8, 0.0, 0.0]),
        "a curve drawn with symmetry off grew a twin"
    );
}

fn place_sphere(document: &mut ClayDocument, at: [f32; 3]) {
    document
        .place_object(
            Shape::Sphere,
            &[0.3],
            at,
            CombineSettings {
                op: Combine::Add,
                ..CombineSettings::default()
            },
        )
        .expect("a placed sphere");
}

/// A placed object is not duplicated by a later symmetry change.
#[test]
fn a_placed_object_is_not_duplicated_by_a_later_symmetry_change() {
    let mut document = document();
    SculptModel::set_symmetry(&mut document, OFF).expect("symmetry off");
    place_sphere(&mut document, [1.8, 0.0, 0.0]);
    assert!(!solid(&document, [-1.8, 0.0, 0.0]), "placed with a twin");

    dab(&mut document, [0.0, 0.0, -1.0], X);
    assert!(
        !solid(&document, [-1.8, 0.0, 0.0]),
        "a sphere placed with symmetry off was duplicated when it went on"
    );
    assert_drawn_as_it_is(&document, -1.8, "the placed sphere's far side");
}

/// Undoing the drag that changed the mirror brings the old images back, and
/// the viewport draws them.
#[test]
fn undoing_a_mirror_change_draws_the_old_images_again() {
    let mut document = document();
    move_the_starting_form_to(&mut document, [1.8, 0.0, 0.0]);
    let before = document.history().depth;
    drag(&mut document, [1.8, 1.0, 0.0], [0.0, 0.2, 0.0], Z);
    assert!(
        !solid(&document, [-1.8, 0.0, 0.0]),
        "the mirror is still across x"
    );

    let recorded = document.history().depth.saturating_sub(before);
    for _ in 0..recorded {
        document.undo().expect("undo");
    }
    assert!(
        solid(&document, [-1.8, 0.0, 0.0]),
        "undoing the change did not bring the twin back"
    );
    assert_drawn_as_it_is(&document, -1.8, "after the undo");
}

/// And one placed with symmetry on is mirrored from the start.
#[test]
fn a_placed_object_is_mirrored_when_symmetry_is_on() {
    let mut document = document();
    dab(&mut document, [0.0, 0.0, -1.0], OFF);
    SculptModel::set_symmetry(&mut document, X).expect("symmetry on");
    place_sphere(&mut document, [1.8, 0.0, 0.0]);
    assert!(
        solid(&document, [-1.8, 0.0, 0.0]),
        "a sphere placed with symmetry on has no twin"
    );
    assert_drawn_as_it_is(&document, -1.8, "the placed sphere's twin");
}

/// A rig with one sphere added one-sided, well clear of the root.
fn rig_with_a_one_sided_sphere(document: &mut ClayDocument) -> NodeIndex {
    document
        .begin_armature([0.0, 0.0, 0.0], 0.3)
        .expect("a rig");
    document
        .add_zsphere(0, [1.0, 0.0, 0.0], 0.2, false)
        .expect("a one-sided sphere")
}

/// A stroke made with symmetry on on the rig's own subtool mirrors the stroke,
/// not the rig: a sphere added one-sided stays one-sided (#170, A5).
#[test]
fn a_stroke_under_symmetry_does_not_mirror_a_one_sided_zsphere() {
    let mut document = document();
    let sphere = rig_with_a_one_sided_sphere(&mut document);
    let twin = [-1.0, 0.0, 0.0];
    assert!(!solid(&document, twin), "added one-sided with a twin");

    dab(&mut document, [0.0, 1.5, 0.0], X);
    assert!(
        !solid(&document, twin),
        "a stroke made with symmetry on gave a one-sided ZSphere a twin"
    );

    // And a rig edit afterwards does not bring the twin in either.
    document
        .move_zsphere(sphere, [0.0, 0.1, 0.0])
        .expect("a rig edit");
    assert!(
        !solid(&document, [-1.0, 0.1, 0.0]),
        "a rig edit mirrored a ZSphere that was added one-sided"
    );
    assert!(solid(&document, [1.0, 0.1, 0.0]), "the sphere did not move");
    assert_drawn_as_it_is(&document, -1.0, "the one-sided sphere's far side");
}

/// A rig edit keeps what was sculpted on the rig's subtool — a carve into the
/// rig included, which a rig re-placed at the end of the layer filled in.
#[test]
fn a_rig_edit_keeps_the_strokes_on_the_rig_layer() {
    let mut document = document();
    let sphere = rig_with_a_one_sided_sphere(&mut document);
    // A lump beside the rig, and a carve into the root sphere.
    dab(&mut document, [0.0, 1.5, 0.0], OFF);
    let carve = [0.0, 0.0, 0.3];
    SculptModel::set_combine(
        &mut document,
        CombineSettings {
            op: Combine::Subtract,
            ..CombineSettings::default()
        },
    );
    let samples: Vec<GestureSample> = (0..3)
        .map(|i| GestureSample {
            position: [carve[0], carve[1] + i as f32 * 0.02, carve[2]],
            pressure: 1.0,
            time: i as f32,
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.2,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            OFF,
        )
        .expect("a carve");
    assert!(!solid(&document, carve), "the carve cut nothing");
    let before = document.history().depth;

    document
        .move_zsphere(sphere, [0.0, 0.1, 0.0])
        .expect("a rig edit");

    assert!(solid(&document, [1.0, 0.1, 0.0]), "the sphere did not move");
    assert!(
        solid(&document, [0.0, 1.5, 0.0]),
        "a rig edit erased a stroke on the rig's subtool"
    );
    assert!(
        !solid(&document, carve),
        "a rig edit filled in a carve made into the rig"
    );
    assert_drawn_as_it_is(&document, 0.0, "the carved rig");

    // Still one undoable action, and it takes back only the edit.
    assert_eq!(
        document.history().depth,
        before + 1,
        "a rig edit is one step"
    );
    document.undo().expect("undo the rig edit");
    assert!(
        solid(&document, [1.0, 0.0, 0.0]),
        "undo did not move it back"
    );
    assert!(!solid(&document, carve), "undo filled in the carve");
}

/// A stroke of `tool` along `path`, with the symmetry the sculptor asked for.
fn stroke(
    document: &mut ClayDocument,
    tool: ToolKind,
    path: &[[f32; 3]],
    size: f32,
    symmetry: [bool; 3],
) {
    SculptModel::set_symmetry(document, symmetry).expect("record the setting");
    let samples: Vec<GestureSample> = path
        .iter()
        .enumerate()
        .map(|(i, at)| GestureSample {
            position: *at,
            pressure: 1.0,
            time: i as f32,
        })
        .collect();
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            symmetry,
        )
        .expect("a stroke");
}

/// The field's value at `at`.
fn field(document: &ClayDocument, at: [f32; 3]) -> f32 {
    document
        .document()
        .eval_points(None, &[at])
        .expect("the field answers")[0]
}

/// A lump sculpted with symmetry on keeps its twin when symmetry is turned
/// off and the next stroke is made one-sided (#170).
#[test]
fn turning_symmetry_off_leaves_mirrored_items_alone() {
    let mut document = document();
    let (lump, twin) = ([1.8, 0.0, 0.0], [-1.8, 0.0, 0.0]);
    dab(&mut document, lump, X);
    assert!(
        solid(&document, twin),
        "a dab made with symmetry on has no twin"
    );

    // Clear of the starting sphere, and off the plane so it could have one.
    let one_sided = [1.0, 1.5, 0.0];
    dab(&mut document, one_sided, OFF);

    assert!(
        solid(&document, twin),
        "turning symmetry off took the twin away from a lump made while it was on"
    );
    assert!(
        solid(&document, one_sided),
        "the one-sided dab deposited nothing"
    );
    assert!(
        !solid(&document, [-1.0, 1.5, 0.0]),
        "a dab made with symmetry off was mirrored by the mirror the layer kept"
    );
    assert_drawn_as_it_is(&document, twin[0], "the kept twin");
    assert_drawn_as_it_is(&document, lump[0], "the lump");
}

/// The same for a pull: a tendril is items, and one pulled with symmetry off
/// stays out of the mirror rather than writing it off.
#[test]
fn a_pull_with_symmetry_off_leaves_mirrored_items_alone() {
    let mut document = document();
    dab(&mut document, [1.8, 0.0, 0.0], X);

    let path: Vec<[f32; 3]> = (0..8).map(|i| [0.8 + i as f32 * 0.05, 1.5, 0.0]).collect();
    stroke(&mut document, ToolKind::Puxar, &path, 0.2, OFF);

    assert!(
        solid(&document, [-1.8, 0.0, 0.0]),
        "a pull with symmetry off took the twin away"
    );
    assert!(solid(&document, [1.0, 1.5, 0.0]), "the pull placed nothing");
    assert!(
        !solid(&document, [-1.0, 1.5, 0.0]),
        "a pull with symmetry off was mirrored"
    );
    assert_drawn_as_it_is(&document, -1.8, "the kept twin");
}

/// A smooth or a flatten with symmetry off keeps the twins made under X, and
/// its bake stays one-sided on the layer that keeps its mirror: nothing on the
/// far side of the plane moves, and the one-sided detail is not copied there.
#[test]
fn a_bake_with_symmetry_off_is_not_copied_across_a_kept_mirror() {
    for tool in [ToolKind::Suavizar, ToolKind::Planar] {
        let mut document = document();
        // Symmetry on first, so the layer carries X.
        dab(&mut document, [1.8, 0.0, 0.0], X);
        SculptModel::set_symmetry(&mut document, OFF).expect("symmetry off");
        place_sphere(&mut document, [0.7, 0.0, 0.8]);
        let far = [-0.7, 0.0, 1.0];
        let before = field(&document, far);
        assert!(!solid(&document, far), "the detail was placed with a twin");

        let path: Vec<[f32; 3]> = (0..6).map(|i| [0.6 + i as f32 * 0.04, 0.0, 1.05]).collect();
        stroke(&mut document, tool, &path, 0.4, OFF);

        let after = field(&document, far);
        assert!(
            (after - before).abs() < 1e-3,
            "a {tool:?} stroke with symmetry off changed the far side of the \
             plane from {before} to {after}"
        );
        assert!(solid(&document, [-1.8, 0.0, 0.0]), "{tool:?} took the twin");
        assert_drawn_as_it_is(&document, -0.7, "the far side of the bake");
    }
}

/// A Move drag with symmetry off moves one side of the starting form only.
/// The form inherits the layer's mirror, and Move writes that mirror off; the
/// lump made under X keeps its twin, since its axes are its own.
#[test]
fn a_drag_with_symmetry_off_moves_one_side() {
    let mut document = document();
    dab(&mut document, [1.8, 0.0, 0.0], X);
    dab(&mut document, [1.0, 1.5, 0.0], OFF);

    let near = [0.6, 0.0, 0.95];
    let far = [-0.6, 0.0, 0.95];
    let (near_before, far_before) = (field(&document, near), field(&document, far));
    let path: Vec<[f32; 3]> = (0..5).map(|i| [0.6, 0.0, 0.8 + i as f32 * 0.05]).collect();
    stroke(&mut document, ToolKind::Mover, &path, 0.35, OFF);

    assert!(
        field(&document, near) < near_before - 1e-3,
        "the drag did not move its own side"
    );
    let far_after = field(&document, far);
    assert!(
        (far_after - far_before).abs() < 1e-3,
        "a drag with symmetry off moved the far side from {far_before} to {far_after}"
    );
    assert_drawn_as_it_is(&document, -0.6, "the far side of the drag");
    assert!(
        solid(&document, [-1.8, 0.0, 0.0]),
        "the drag with symmetry off took the twin away from a lump made under X"
    );
}

/// Switching symmetry to another axis leaves what was made under the old one
/// exactly as it was: the lump keeps its X twin and gains no Z twin, through a
/// stroke and through the drag that re-points the layer's mirror.
#[test]
fn switching_the_axis_leaves_items_made_under_the_old_axis_unchanged() {
    let mut document = document();
    let (lump, twin, across_z) = ([1.8, 0.0, 0.4], [-1.8, 0.0, 0.4], [1.8, 0.0, -0.4]);
    dab(&mut document, lump, X);
    assert!(solid(&document, twin), "the lump has no X twin");
    assert!(!solid(&document, across_z), "the lump has a Z twin already");

    dab(&mut document, [1.0, 1.5, 0.5], Z);
    assert!(
        solid(&document, [1.0, 1.5, -0.5]),
        "the dab made under Z is not mirrored across z"
    );
    drag(&mut document, [0.6, 0.0, 0.8], [0.0, 0.0, 0.2], Z);

    assert!(
        solid(&document, twin),
        "switching symmetry to Z took the X twin away from a lump made under X"
    );
    assert!(
        !solid(&document, across_z),
        "switching symmetry to Z gave a lump made under X a Z twin"
    );
    assert_drawn_as_it_is(&document, twin[0], "the kept X twin");
    assert_drawn_as_it_is(&document, lump[0], "the lump");
}

/// Turning symmetry off leaves a mirrored item mirrored, through the two
/// verbs that write the layer's mirror off: a Move and a Pinçar made with
/// symmetry off.
#[test]
fn turning_symmetry_off_leaves_a_mirrored_item_mirrored() {
    let mut document = document();
    let (lump, twin) = ([1.8, 0.0, 0.0], [-1.8, 0.0, 0.0]);
    dab(&mut document, lump, X);
    assert!(solid(&document, twin), "the lump has no twin");

    drag(&mut document, [0.6, 0.0, 0.8], [0.0, 0.0, 0.2], OFF);
    assert!(
        solid(&document, twin),
        "a Move with symmetry off took the twin away from a lump made under X"
    );

    let path: Vec<[f32; 3]> = (0..4).map(|i| [0.0, 0.6 + i as f32 * 0.05, 0.8]).collect();
    stroke(&mut document, ToolKind::Pincar, &path, 0.3, OFF);
    assert!(
        solid(&document, twin),
        "a Pinçar with symmetry off took the twin away from a lump made under X"
    );
    assert_drawn_as_it_is(&document, twin[0], "the kept twin");
    assert_drawn_as_it_is(&document, lump[0], "the lump");
}

/// An item made under symmetry is one item with its reflections, and a drag
/// that reaches it moves them together whatever symmetry the drag is made
/// with — the engine's rule for an item carrying its own axes, pinned so a
/// change of it is noticed. The starting form, which inherits the layer's
/// mirror, is the one a drag with symmetry off moves on one side.
#[test]
fn a_drag_with_symmetry_off_on_an_item_made_under_symmetry_moves_both_images() {
    let mut document = document();
    dab(&mut document, [1.8, 0.0, 0.0], X);
    let (near, far) = ([1.8, 0.0, 0.5], [-1.8, 0.0, 0.5]);
    let (near_before, far_before) = (field(&document, near), field(&document, far));
    assert!(
        (near_before - far_before).abs() < 1e-3,
        "the lump and its twin differ before the drag"
    );

    drag(&mut document, [1.8, 0.0, 0.25], [0.0, 0.0, 0.2], OFF);

    let (near_after, far_after) = (field(&document, near), field(&document, far));
    assert!(
        near_after < near_before - 1e-3,
        "the drag did not move the lump: {near_before} to {near_after}"
    );
    assert!(
        (near_after - far_after).abs() < 1e-2,
        "the drag moved the lump to {near_after} and its twin to {far_after}: an \
         item made under X is reached through its own reflections, so both \
         images follow a drag together"
    );
    assert_drawn_as_it_is(&document, -1.8, "the twin after the drag");
}

/// An item made under symmetry is mirrored through the layer's planes even
/// where the layer's own mirror has never been pointed: a fresh subtool's
/// engine mirror is off, and a lump made on it under X still has its twin.
#[test]
fn an_item_made_under_symmetry_is_mirrored_on_a_layer_whose_mirror_is_off() {
    let mut document = document();
    let key = document
        .add_layer("Lado", Representation::Sdf)
        .expect("a second subtool");
    let layer = document.layer_id(key).expect("its engine id");
    dab(&mut document, [1.8, 0.0, 0.0], X);

    let (carried, _) = document
        .document()
        .layer_mirror(layer)
        .expect("the layer answers");
    assert_eq!(carried, OFF, "a stroke wrote the layer's mirror");
    assert!(
        solid(&document, [-1.8, 0.0, 0.0]),
        "a lump made under X on a layer whose mirror is off has no twin"
    );
    assert_drawn_as_it_is(&document, -1.8, "the twin on the fresh subtool");
}
