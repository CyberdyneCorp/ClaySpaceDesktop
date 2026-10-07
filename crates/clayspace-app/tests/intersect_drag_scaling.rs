//! What an intersect drag frame refills on a worked form, at two sizes (#282).
//!
//! `clayspace-engine/tests/intersect_drag.rs` checks the drag on the bare
//! starting form. This file uses the benchmark's own scenes, where 96 blended
//! stroke stamps precede the operand. Until ClayCore v0.126.0 the engine
//! padded the move's surface-delta box by the blend support of the whole
//! layer, which on a worked form scales with the form, so a frame refilled
//! 3,360 keys on `reference` and 23,520 to 26,880 on `reference-10x` against
//! 1,012 for the subtracting control (ClayCore#666). The pad is now the sum of
//! the blend supports of the combines *after* the operand, and a placed
//! operand is appended last, so it carries no pad: the box is the sweep alone,
//! and a frame refills what the subtracting control refills at either size.
//!
//! Brick keys rather than milliseconds, so the figure is deterministic. The
//! promise is held against the control rather than the layer, because the
//! layer grows with the form and the sweep does not: a regression to the old
//! pad would still refill under half the layer on `reference-10x`.
//!
//! Measured on this engine pin (ClayCore v0.126.0), keys per frame:
//!
//! | scene | layer | subtract, three frames | intersect, the same frames |
//! |---|---:|---:|---:|
//! | reference | 5,040 | 880 / 1,056 / 1,232 | 880 / 1,056 / 1,232 |
//! | reference-10x | 84,672 | 880 / 1,056 / 1,232 | 880 / 1,056 / 1,232 |
//!
//! The frames print their wall time as well, for the issue's second criterion
//! (an intersect frame on `reference-10x` refilled for 6.8 to 10.1 seconds at
//! v0.120.1), but the assertion is on keys. On an Apple M3 Pro with Metal at a
//! load of about 9 across 12 cores, the three `reference-10x` frames took
//! 13.9 / 125.0 / 26.7 ms intersecting against 14.8 / 15.7 / 17.2 ms
//! subtracting, and the `reference` frames 6.1 / 10.1 / 6.6 against 4.3 /
//! 4.2 / 4.6. The region is the same; what the engine's fill spends per brick
//! of an intersect on the large form is not, on some frames, and that is a
//! cost of the fill rather than of the bound.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Combine, CombineSettings, GizmoDrag, GizmoHandle, GizmoMode, GizmoTarget, ObjectId,
    ObjectModel, Shape,
};

/// The benchmark's operand and where it is placed.
const CUT: [f32; 2] = [0.25, 1.6];
const AT: [f32; 3] = [0.0, 0.9, 0.0];

/// How many frames of the drag to measure.
const FRAMES: usize = 3;

/// A scene with the operand placed, and the keys the placement refilled. For
/// an intersect that is the whole layer, since its influence bound is the
/// layer.
fn placed(scene: Scene, op: Combine) -> (ClayDocument, ObjectId, usize) {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut doc = scene.build(policy).expect("the scene builds");
    doc.take_dirty_keys();
    let settings = CombineSettings {
        op,
        ..CombineSettings::default()
    };
    let id = doc
        .place_object(Shape::Cylinder, &CUT, AT, settings)
        .expect("place the operand");
    let refilled = doc.take_dirty_keys().len();
    (doc, id, refilled)
}

/// One frame of the benchmark's drag: the keys it refilled and what the move
/// took, refill included.
struct Frame {
    keys: usize,
    millis: f64,
}

/// The frames of the benchmark's drag across the form, as the viewport would
/// apply them.
fn drag(doc: &mut ClayDocument, id: ObjectId) -> Vec<Frame> {
    let target = GizmoTarget::Object(id);
    let start = doc.target_transform(target).expect("a transform");
    let gesture = GizmoDrag {
        mode: GizmoMode::Move,
        handle: GizmoHandle::Axis(0),
        pivot: start.position,
        anchor: start.position,
        view_axis: [0.0, 0.0, 1.0],
    };
    doc.begin_target_drag(target);
    let frames = (1..=FRAMES)
        .map(|step| {
            let t = step as f32 / 12.0;
            let to = [(t * std::f32::consts::TAU).sin() * 0.7, 0.9, 0.0];
            let started = Instant::now();
            doc.set_target_transform(target, gesture.resolve(start, to, false))
                .expect("a drag frame");
            let millis = started.elapsed().as_secs_f64() * 1e3;
            let keys = doc.take_dirty_keys().len();
            Frame { keys, millis }
        })
        .collect();
    doc.end_target_drag();
    frames
}

/// The same drag with both operations on one scene, frame by frame, and the
/// promise: an intersect frame refills no more than the subtracting control
/// does, and never its layer. The two are equal on this pin — the sweep is
/// the control's own box — and the bound is one-sided so a narrower engine
/// answer passes while any pad fails.
fn assert_intersect_costs_its_sweep(scene: Scene) {
    let (mut intersect, a, layer) = placed(scene, Combine::Intersect);
    let (mut subtract, b, _) = placed(scene, Combine::Subtract);
    let crossed = drag(&mut intersect, a);
    let control = drag(&mut subtract, b);

    let name = scene.member();
    for (frame, (cut, sub)) in crossed.iter().zip(&control).enumerate() {
        println!(
            "{name} frame {frame}: intersect {} keys in {:.1} ms, subtract {} keys in \
             {:.1} ms, the layer is {layer} keys",
            cut.keys, cut.millis, sub.keys, sub.millis
        );
        assert!(sub.keys > 0, "the control frame refilled nothing");
        assert!(
            cut.keys < layer,
            "an intersect frame refilled {} keys, its whole layer of {layer}",
            cut.keys
        );
        assert!(
            cut.keys <= sub.keys,
            "an intersect frame refilled {} keys against {} subtracting, so the \
             move is padded by more than the combines after its operand again",
            cut.keys,
            sub.keys
        );
    }
}

/// On the scene with ten times the surface, a frame refilled the whole layer
/// (84,672 keys, seconds a frame) and then about a quarter of it (the old
/// pad). It now refills the sweep: 880 to 1,232 keys over the three frames,
/// the same as the subtracting control.
#[test]
fn an_intersect_drag_on_a_large_form_refills_what_its_subtract_control_does() {
    assert_intersect_costs_its_sweep(Scene::TenTimesLarger);
}

/// And on the reference scene, where the old pad put a frame at 3,360 keys
/// against the control's 1,012.
#[test]
fn an_intersect_drag_on_the_reference_form_refills_what_its_subtract_control_does() {
    assert_intersect_costs_its_sweep(Scene::Reference);
}
