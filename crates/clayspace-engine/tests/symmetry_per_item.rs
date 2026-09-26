//! Symmetry mirrors what is made while it is on, not what was already there.
//!
//! The engine's mirror is a property of the *layer*: `clay_set_layer_mirror`
//! reflects every item the layer holds, past and future. Read naively, that
//! makes the symmetry switch reach backwards — a lump sculpted one-sided grew a
//! twin the moment the next stroke wrote the mirror, and a box placed with
//! symmetry off became two when it was turned on (#170, Y2).
//!
//! Each item says whether it takes part (`clay_item_set_mirror`), so the
//! answer is to decide that when the item is made: made with symmetry off, it
//! stays out of every mirror the layer is given later.
//!
//! And a mirror change moves surface the brick cache holds. The images the old
//! mirror made are gone from the field, and unless the bricks under them are
//! refilled the viewport keeps drawing them — on a hidden layer too, because
//! hiding refills only what the layer reaches *now* (#170, Y1).

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, CurveModel, GestureSample, ObjectModel,
    Representation, SceneModel, SculptModel, Shape, ToolKind,
};

const OFF: [bool; 3] = [false; 3];
const X: [bool; 3] = [true, false, false];

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

/// A lump sculpted one-sided stays one-sided when symmetry is turned on and
/// the next stroke writes the mirror.
#[test]
fn turning_symmetry_on_leaves_existing_items_alone() {
    let mut document = document();
    // Well clear of the starting sphere, so the lump is the only thing there.
    let lump = [1.8, 0.0, 0.0];
    let twin = [-1.8, 0.0, 0.0];
    dab(&mut document, lump, OFF);
    assert!(solid(&document, lump), "the dab deposited nothing");
    assert!(!solid(&document, twin), "an unmirrored dab has a twin");

    // Symmetry on, and a stroke on the plane, which is what writes the mirror.
    dab(&mut document, [0.0, 0.0, -1.0], X);

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
#[test]
fn a_mirror_change_dirties_both_images() {
    let mut document = document();
    dab(&mut document, [1.8, 0.0, 0.0], X);
    assert!(solid(&document, [-1.8, 0.0, 0.0]), "no twin to take away");
    assert_drawn_as_it_is(&document, -1.8, "before the change");

    // Symmetry off: the next stroke writes the mirror as off, which takes the
    // twin out of the field.
    dab(&mut document, [0.0, 0.0, -1.0], OFF);

    assert_drawn_as_it_is(&document, -1.8, "after the mirror went off");
    assert_drawn_as_it_is(&document, 1.8, "the stroke's own side");
}

/// Hidden after a mirror change, a layer leaves nothing behind.
#[test]
fn a_hidden_layer_draws_nothing_after_a_mirror_change() {
    let mut document = document();
    let key = document
        .add_layer("Lado", Representation::Sdf)
        .expect("a second subtool");
    dab(&mut document, [1.8, 0.0, 0.0], X);
    dab(&mut document, [1.8, 0.0, 0.4], OFF);

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

/// Undoing the stroke that changed the mirror brings the old images back, and
/// the viewport draws them.
#[test]
fn undoing_a_mirror_change_draws_the_old_images_again() {
    let mut document = document();
    dab(&mut document, [1.8, 0.0, 0.0], X);
    let before = document.history().depth;
    dab(&mut document, [0.0, 0.0, -1.0], OFF);
    assert!(
        !solid(&document, [-1.8, 0.0, 0.0]),
        "the mirror is still on"
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
