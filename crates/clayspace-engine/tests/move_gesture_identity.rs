//! Whether two Move drags stay two drags.
//!
//! The engine folds a grab into the one leading an item's chain when it decides
//! both belong to the drag in progress, and the fold *replaces*: a drag re-sends
//! its whole displacement from the anchor, so the earlier frame is superseded
//! rather than stacked. With no gesture named it decides by centre and radius
//! compared bit for bit, so a second drag pressed where the first was pressed,
//! at the same size, is taken for the first one continuing — and the first
//! drag's pull is gone. Naming each gesture is what keeps them apart (#122).
//!
//! Both doors: the live transaction a drag takes when it can, and the held path
//! it takes when it cannot.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SceneModel, SculptModel, ToolKind};

const OUTWARD: [f32; 3] = [1.0, 0.0, 0.0];

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far the surface stands from the origin along `direction`.
fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> f32 {
    let origin = direction.map(|c| c * 4.0);
    let hit = document
        .pick(origin, direction.map(|c| -c))
        .expect("the ray reaches the surface");
    hit.iter().map(|c| c * c).sum::<f32>().sqrt()
}

/// The deepest deformer chain on the active layer.
fn chain(document: &ClayDocument) -> i32 {
    let key = document.scene().active_layer().expect("a layer").key;
    let id = document.layer_id(key).expect("the layer's engine id");
    document
        .document()
        .field_report(id, 0.5)
        .expect("a report")
        .longest_deformer_chain
}

/// The path of the drag, from its anchor to `step` of six.
fn path(step: usize) -> Vec<GestureSample> {
    (0..=step)
        .map(|i| {
            let t = i as f32 / 6.0;
            GestureSample {
                position: [1.0 + t * 0.25, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.35,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// One live drag in the ViewModel's order: opened, begun, sent in segments,
/// committed, ended.
fn live_drag(document: &mut ClayDocument) {
    assert!(
        document.open_live_gesture(ToolKind::Mover, [false; 3]),
        "an editable field subtool takes a live drag"
    );
    SculptModel::begin_gesture(document);
    for step in 1..=6 {
        document
            .apply_stroke(ToolKind::Mover, brush(), &path(step), [false; 3])
            .expect("the drag was refused");
    }
    document.close_live_gesture().expect("commit");
    SculptModel::end_gesture(document);
}

/// One held drag: begun, sent whole, ended.
fn held_drag(document: &mut ClayDocument) {
    SculptModel::begin_gesture(document);
    document
        .apply_stroke(ToolKind::Mover, brush(), &path(6), [false; 3])
        .expect("the drag was refused");
    SculptModel::end_gesture(document);
}

/// What two drags from the same press leave behind.
struct TwoDrags {
    rest: f32,
    once: f32,
    twice: f32,
    unwarped: i32,
    after_one: i32,
    after_two: i32,
}

impl TwoDrags {
    fn measure(drag: fn(&mut ClayDocument)) -> Self {
        let mut document = sphere();
        let rest = radius_along(&document, OUTWARD);
        let unwarped = chain(&document);
        drag(&mut document);
        let (once, after_one) = (radius_along(&document, OUTWARD), chain(&document));
        drag(&mut document);
        let (twice, after_two) = (radius_along(&document, OUTWARD), chain(&document));
        let measured = Self {
            rest,
            once,
            twice,
            unwarped,
            after_one,
            after_two,
        };
        eprintln!(
            "rest {rest:.4}, one drag {once:.4}, two {twice:.4}; \
             chain {unwarped} -> {after_one} -> {after_two}"
        );
        // Without a first pull there is nothing for the second to lose, and
        // without one warp per drag the counts below say nothing.
        assert!(
            once - rest > 0.02,
            "the first drag did not pull: {rest:.4} -> {once:.4}"
        );
        assert_eq!(
            after_one,
            unwarped + 1,
            "one drag, sent in segments, is one warp"
        );
        measured
    }

    /// The fold replaces, so a folded second drag leaves the surface exactly
    /// where one drag left it and the chain where one drag left it.
    fn folded(&self) -> bool {
        self.after_two == self.after_one && self.twice == self.once
    }

    fn added(&self) -> bool {
        self.after_two == self.unwarped + 2
            && self.twice > self.once + 0.25 * (self.once - self.rest)
    }
}

#[test]
fn a_second_held_drag_from_the_same_press_adds_to_the_first() {
    let drags = TwoDrags::measure(held_drag);
    assert!(
        drags.added(),
        "the second drag folded into the first: pressed at the same point at the \
         same size, it was taken for the first one continuing"
    );
}

/// A TRIPWIRE, not the property: on this pin the live door cannot name its
/// gesture, so its second drag still replaces its first.
///
/// `clay_sdf_move_begin` in ClayCore v0.113.0 reads `clay_move_params` and
/// copies `radius`, `ease` and `front_only` into the transaction's settings,
/// and not `gesture_id`, which `clay_layer_move_surface` does copy. The id this
/// application sends is correct and never arrives. This fails the day a pin
/// carries the fix; turn it into the held door's assertion then.
#[test]
fn a_second_live_drag_from_the_same_press_still_replaces_the_first_on_this_pin() {
    let drags = TwoDrags::measure(live_drag);
    assert!(
        drags.folded(),
        "the live door kept two drags apart, so the pinned engine now carries \
         gesture_id through clay_sdf_move_begin: assert `added()` here as the \
         held door does"
    );
}

/// Rest, near side and far side of a mirrored drag sent in anchored segments.
fn mirrored_held_drag(named: bool) -> (f32, f32, f32) {
    const MIRROR_X: [bool; 3] = [true, false, false];
    let mut document = sphere();
    let rest = radius_along(&document, OUTWARD);
    if named {
        SculptModel::begin_gesture(&mut document);
    }
    for step in 1..=6 {
        document
            .apply_stroke(ToolKind::Mover, brush(), &path(step), MIRROR_X)
            .expect("the drag was refused");
    }
    if named {
        SculptModel::end_gesture(&mut document);
    }
    let near = radius_along(&document, OUTWARD);
    let far = radius_along(&document, OUTWARD.map(|c| -c));
    (rest, near, far)
}

/// A GUARD rather than a regression: naming must not cost a mirror its far side.
///
/// The held door calls the engine once per mirror image, and every image
/// carries the gesture's one name. A named grab folds into any leading grab of
/// that name, so were one image's grab able to replace the other's, a mirrored
/// drag would pull one side only. Compared against the unnamed drag, where each
/// image keeps its own centre and cannot be taken for the other.
#[test]
fn naming_a_mirrored_held_drag_keeps_both_sides() {
    let (rest, near_unnamed, far_unnamed) = mirrored_held_drag(false);
    let (_, near_named, far_named) = mirrored_held_drag(true);
    eprintln!(
        "rest {rest:.4}; unnamed near {near_unnamed:.4} far {far_unnamed:.4}; \
         named near {near_named:.4} far {far_named:.4}"
    );
    assert!(
        far_unnamed - rest > 0.02,
        "the mirrored drag did not reach the far side, so this compares nothing"
    );
    assert!(
        (near_named - near_unnamed).abs() < 1e-4 && (far_named - far_unnamed).abs() < 1e-4,
        "naming the gesture changed a mirrored drag: near {near_unnamed:.4} -> \
         {near_named:.4}, far {far_unnamed:.4} -> {far_named:.4}"
    );
}
