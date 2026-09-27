//! What an intersect drag frame refills on a worked form, at two sizes (#282).
//!
//! `clayspace-engine/tests/intersect_drag.rs` checks the drag on the bare
//! starting form, where the sweep and the subtracting control come out the
//! same. This file uses the benchmark's own scenes, where 96 blended stroke
//! stamps give the engine's surface-delta box a chain pad. There the delta box
//! is larger than the sweep and can reach past the layer, so a frame refills
//! the overlap of that box with the influence bound. What the fix promises
//! here is that a frame refills less than the layer, and far less once the
//! layer is large. Brick keys rather than milliseconds, so the figure is
//! deterministic.
//!
//! Measured when this was written (ClayCore v0.120.1), keys per frame:
//!
//! | scene | subtract | intersect, layer | intersect, now |
//! |---|---:|---:|---:|
//! | reference | 1,012 | 5,040 | 3,360 |
//! | reference-10x | 1,012 | 84,672 | 23,520–26,880 |
//!
//! The rest of the gap to the subtracting control is the engine's chain pad
//! (about 0.47 at the reference size, 1.48 at ten times it), tracked in
//! CyberdyneCorp/ClayCore#666.

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Combine, CombineSettings, GizmoDrag, GizmoHandle, GizmoMode, GizmoTarget, ObjectId,
    ObjectModel, Shape,
};

/// The benchmark's operand and where it is placed.
const CUT: [f32; 2] = [0.25, 1.6];
const AT: [f32; 3] = [0.0, 0.9, 0.0];

/// A scene with an intersecting operand placed, and the keys the placement
/// refilled. For an intersect that is the whole layer, since its influence
/// bound is the layer.
fn placed(scene: Scene) -> (ClayDocument, ObjectId, usize) {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut doc = scene.build(policy).expect("the scene builds");
    doc.take_dirty_keys();
    let intersecting = CombineSettings {
        op: Combine::Intersect,
        ..CombineSettings::default()
    };
    let id = doc
        .place_object(Shape::Cylinder, &CUT, AT, intersecting)
        .expect("place the operand");
    let layer = doc.take_dirty_keys().len();
    (doc, id, layer)
}

/// The keys each frame of the benchmark's drag refills, across the form.
fn drag_keys(doc: &mut ClayDocument, id: ObjectId, frames: usize) -> Vec<usize> {
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
    let keys = (1..=frames)
        .map(|step| {
            let t = step as f32 / 12.0;
            let to = [(t * std::f32::consts::TAU).sin() * 0.7, 0.9, 0.0];
            doc.set_target_transform(target, gesture.resolve(start, to, false))
                .expect("a drag frame");
            doc.take_dirty_keys().len()
        })
        .collect();
    doc.end_target_drag();
    keys
}

/// On the scene with ten times the surface, a frame refilled the whole layer:
/// 84,672 keys, about a second a frame. It now refills the sweep the engine
/// reports.
#[test]
fn an_intersect_drag_on_a_large_form_refills_well_under_its_layer() {
    let (mut doc, id, layer) = placed(Scene::TenTimesLarger);
    let frames = drag_keys(&mut doc, id, 3);
    println!("reference-10x: the layer is {layer} keys, drag frames {frames:?}");

    assert!(layer > 0, "placing the operand refilled nothing");
    for keys in frames {
        assert!(
            keys * 2 < layer,
            "a drag frame refilled {keys} of the layer's {layer} keys, so the \
             move is still refilling by the intersect's influence bound"
        );
    }
}

/// And less than the layer on the reference scene, where the delta box alone
/// reaches past it: 5,400 keys on one frame against the layer's 5,040.
#[test]
fn an_intersect_drag_on_the_reference_form_refills_less_than_its_layer() {
    let (mut doc, id, layer) = placed(Scene::Reference);
    let frames = drag_keys(&mut doc, id, 3);
    println!("reference: the layer is {layer} keys, drag frames {frames:?}");

    for keys in frames {
        assert!(
            keys * 4 < layer * 3,
            "a drag frame refilled {keys} keys against the layer's {layer}, so \
             it is not held inside the influence bound"
        );
    }
}
