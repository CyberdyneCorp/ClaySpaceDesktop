//! What a placed armature reads back.
//!
//! Filed as ClayCore #77 when the answer was "nothing", and fixed in 0.29.0:
//! `clay_layer_stroke_points` now serves the xyzr half of an armature instead
//! of refusing it, and `clay_layer_armature_parents` is the topology half.
//!
//! These now assert the fix rather than the defect, which is what stops a
//! regression going unnoticed — a test deleted the day its bug is fixed
//! protects nothing.

use clayspace_engine::claycore::{Document, Item, LayerId, NodeId, Op};

/// A rig that BRANCHES, which is the case no guess can recover: node 3 hangs
/// off node 1, not off node 2.
const POINTS: [f32; 16] = [
    0.0, 0.0, 0.0, 0.30, // 0, the root
    0.5, 0.0, 0.0, 0.20, // 1, off the root
    1.0, 0.0, 0.0, 0.15, // 2, off 1
    0.5, 0.6, 0.0, 0.15, // 3, off 1 as well — the branch
];
const PARENTS: [u32; 4] = [0, 0, 1, 1];

fn authored() -> (Document, LayerId, NodeId) {
    let mut document = Document::new().expect("a document");
    let layer = document.add_sdf_layer("Rig").expect("a layer");
    let mut item = Item::armature().expect("an armature item");
    item.set_stroke_points(&POINTS).expect("points");
    item.set_armature_parents(&PARENTS).expect("parents");
    item.set_op(Op::Add).expect("op");
    let node = document.add_item(layer, &item).expect("place");
    (document, layer, node)
}

#[test]
fn a_placed_armature_reads_back_the_points_it_was_authored_with() {
    let (document, layer, node) = authored();
    let points = document
        .stroke_points(layer, node)
        .expect("an armature's points are readable since 0.29.0");
    assert_eq!(points.len(), 4);
    for (i, point) in points.iter().enumerate() {
        for c in 0..4 {
            assert!(
                (point[c] - POINTS[i * 4 + c]).abs() < 1e-6,
                "point {i} channel {c}: {} against {}",
                point[c],
                POINTS[i * 4 + c]
            );
        }
    }
}

#[test]
fn a_stroke_on_the_same_call_answers_normally() {
    // The contrast that makes the gap a gap rather than a design: the same
    // call, on the primitive it was written for, round trips exactly as its
    // documentation promises.
    let (mut document, layer, _) = authored();
    let mut stroke = Item::stroke().expect("a stroke item");
    let points = [0.0f32, 2.0, 0.0, 0.2, 0.4, 2.0, 0.0, 0.2];
    stroke.set_stroke_points(&points).expect("points");
    stroke.set_op(Op::Add).expect("op");
    let node = document.add_item(layer, &stroke).expect("add the stroke");

    let read = document.stroke_points(layer, node).expect("stroke points");
    assert_eq!(read.len(), 2);
    for (i, point) in read.iter().enumerate() {
        for c in 0..4 {
            assert!(
                (point[c] - points[i * 4 + c]).abs() < 1e-6,
                "point {i} channel {c}: {} against {}",
                point[c],
                points[i * 4 + c]
            );
        }
    }
}

#[test]
fn the_topology_reads_back_too_including_a_branch() {
    // The half that made the difference. Positions alone cannot be turned back
    // into a rig: node 3 hangs off node 1, not node 2, and no amount of
    // nearest-sphere guessing recovers that.
    let (document, layer, node) = authored();
    let parents = document
        .armature_parents(layer, node)
        .expect("the parent array is readable since 0.29.0");
    assert_eq!(
        parents, PARENTS,
        "the branch did not survive the round trip"
    );
}

#[test]
fn a_reopened_document_can_tell_an_armature_from_a_stroke() {
    // The other half of finding a rig again: `clay_layer_node_prim` says what
    // a placed node carries, which nothing in the ABI answered before.
    let (mut document, layer, armature) = authored();
    let mut stroke = Item::stroke().expect("a stroke item");
    stroke
        .set_stroke_points(&[0.0, 2.0, 0.0, 0.2, 0.4, 2.0, 0.0, 0.2])
        .expect("points");
    stroke.set_op(Op::Add).expect("op");
    let stroke_node = document.add_item(layer, &stroke).expect("add");

    assert_ne!(
        document.node_prim(layer, armature).expect("armature prim"),
        document.node_prim(layer, stroke_node).expect("stroke prim"),
        "a rig and a stroke report the same primitive"
    );
}
