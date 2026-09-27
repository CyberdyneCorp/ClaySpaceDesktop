//! What one frame of a live intersect drag refills (#282).
//!
//! An operand placed with `Combine::Intersect` has the whole layer as its
//! influence bound — `max(acc, item)` is the item's own value everywhere the
//! item is not, so an arbitrary edit to it really can change the field
//! anywhere the layer has material. A *move* cannot: outside the swept union of
//! where the operand was and where it went, the band-clamped field is the same
//! on both sides. The engine answers that narrower question through
//! `clay_layer_set_transform_bound`, and a drag that asked the influence bound
//! instead refilled the whole layer every frame — 2.7x the same drag with
//! `Combine::Subtract` on the reference scene, seconds a frame at ten times
//! its extent.
//!
//! Two promises, and the second is what keeps the first honest: a frame
//! refills about what the subtracting control does, and what it leaves behind
//! is the surface a document built with the operand already there holds.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Combine, CombineSettings, GizmoTarget, ObjectId, ObjectModel, SculptModel, Shape, Transform,
};

/// Big enough to reach the surface it cuts, small enough that its box is not
/// the whole form — the benchmark's operand.
const CUT: [f32; 2] = [0.25, 1.6];
const START: [f32; 3] = [0.0, 0.9, 0.0];

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

fn combining(op: Combine) -> CombineSettings {
    CombineSettings {
        op,
        ..CombineSettings::default()
    }
}

/// The starting form with the operand placed, and the placement's refill
/// already taken.
fn placed(op: Combine, scale: [f32; 3]) -> (ClayDocument, ObjectId) {
    let mut doc = document();
    let id = doc
        .place_object(Shape::Cylinder, &CUT, START, combining(op))
        .expect("place the operand");
    if scale != [1.0; 3] {
        doc.set_object_transform(id, START, [0.0, 1.0, 0.0], 0.0, scale)
            .expect("stretch the operand");
    }
    doc.take_dirty_keys();
    (doc, id)
}

/// A few frames of a drag across the form, each refilled as it lands, as the
/// viewport would. Returns the bricks each frame dirtied.
fn drag(doc: &mut ClayDocument, id: ObjectId, scale: [f32; 3], to: &[[f32; 3]]) -> Vec<usize> {
    let target = GizmoTarget::Object(id);
    doc.begin_target_drag(target);
    let frames = to
        .iter()
        .map(|&position| {
            let moved = Transform {
                position,
                rotation_axis: [0.0, 1.0, 0.0],
                rotation_angle: 0.0,
                scale,
            };
            doc.set_target_transform(target, moved)
                .expect("a drag frame");
            doc.take_dirty_keys().len()
        })
        .collect();
    doc.end_target_drag();
    frames
}

const FRAMES: [[f32; 3]; 4] = [
    [0.1, 0.9, 0.0],
    [0.2, 0.9, 0.0],
    [0.3, 0.9, 0.0],
    [0.25, 0.9, 0.0],
];

/// The regression: an intersect frame refilled the whole layer, and now it
/// refills about what the subtracting control does.
#[test]
fn an_intersect_drag_frame_refills_about_what_a_subtract_frame_does() {
    let (mut intersect, a) = placed(Combine::Intersect, [1.0; 3]);
    let (mut subtract, b) = placed(Combine::Subtract, [1.0; 3]);

    let crossed: usize = drag(&mut intersect, a, [1.0; 3], &FRAMES).iter().sum();
    let control: usize = drag(&mut subtract, b, [1.0; 3], &FRAMES).iter().sum();
    println!("intersect frames dirtied {crossed} bricks, subtract frames {control}");

    assert!(control > 0, "the control drag dirtied nothing");
    assert!(
        (crossed as f64) < 1.5 * control as f64,
        "an intersect drag dirtied {crossed} bricks against {control} for the \
         same drag subtracting, so it is still refilling the layer every frame"
    );
}

/// And the narrow refill is not a stale one: after the drag, the cache the
/// viewport meshes puts the surface where the document does, over the whole
/// path the operand swept — including the place it left.
#[test]
fn an_intersect_drag_leaves_no_stale_surface_behind() {
    let (mut dragged, id) = placed(Combine::Intersect, [1.0; 3]);
    drag(&mut dragged, id, [1.0; 3], &FRAMES);
    assert_cache_agrees(&dragged);
}

/// A stretched operand has no fast path in the engine and keeps the per-axis
/// write: it stays stretched, and its drag is still correct.
#[test]
fn a_stretched_intersect_operand_stays_stretched_and_correct() {
    let stretch = [1.4, 1.0, 1.0];
    let (mut dragged, id) = placed(Combine::Intersect, stretch);
    drag(&mut dragged, id, stretch, &FRAMES);

    let now = dragged
        .target_transform(GizmoTarget::Object(id))
        .expect("a transform");
    assert_eq!(now.scale, stretch, "the drag unsquashed the operand");
    assert_cache_agrees(&dragged);
}

/// A voxel is 0.02, so agreement closer than this is agreement.
const VOXEL: f32 = 0.02;

/// Rays down onto the form across everything the drag swept, each asked of
/// the document and of the cache. A brick left holding the operand where it
/// used to stand answers the cache's ray and not the document's.
fn assert_cache_agrees(doc: &ClayDocument) {
    let mut checked = 0;
    for i in -8..=8 {
        for k in -4..=4 {
            let origin = [i as f32 * 0.075, 4.0, k as f32 * 0.075];
            let down = [0.0, -1.0, 0.0];
            let truth = doc.document().raycast(origin, down).ok().flatten();
            let seen = doc.cache().raycast(origin, down).ok().flatten();
            match (truth, seen) {
                (None, None) => {}
                (Some(truth), Some(seen)) => {
                    let (truth, seen) = (truth.position[1], seen.position[1]);
                    assert!(
                        (truth - seen).abs() < VOXEL,
                        "at x {} z {} the document puts the surface at {truth} and \
                         the cache at {seen}: the drag left a stale brick",
                        origin[0],
                        origin[2]
                    );
                    checked += 1;
                }
                (truth, seen) => panic!(
                    "at x {} z {} the document hits {truth:?} and the cache {seen:?}",
                    origin[0], origin[2]
                ),
            }
        }
    }
    assert!(checked > 0, "no ray met the form, so nothing was compared");
}

/// Placing an intersect operand keeps only what it intersects. It was added at
/// the origin and then moved into place — two engine edits with no refill
/// between — and the engine's seeds kept the whole un-intersected form outside
/// the cut however much of it was dirtied.
#[test]
fn placing_an_intersect_operand_leaves_no_uncut_form_behind() {
    let (doc, _) = placed(Combine::Intersect, [1.0; 3]);
    assert_cache_agrees(&doc);
}

/// Taking the drag back, and giving it again, lands on surfaces the cache
/// agrees with: the history steps replay the moves the narrow bound covered.
#[test]
fn undoing_and_redoing_an_intersect_drag_leaves_no_stale_surface() {
    let (mut doc, id) = placed(Combine::Intersect, [1.0; 3]);
    drag(&mut doc, id, [1.0; 3], &FRAMES);

    assert!(doc.undo().expect("undo the drag"), "nothing to undo");
    assert_cache_agrees(&doc);
    assert!(doc.redo().expect("redo the drag"), "nothing to redo");
    assert_cache_agrees(&doc);
}
