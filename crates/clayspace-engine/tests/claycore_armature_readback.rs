//! An armature is write-only. A repro against the C ABI, not a test of our
//! model, so it says exactly what upstream would have to reproduce.
//!
//! Kept isolated from the application's own types for the same reason.

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

fn authored() -> Option<(Document, LayerId, NodeId)> {
    let mut document = Document::new().ok()?;
    let layer = document.add_sdf_layer("Rig").ok()?;
    let mut item = Item::armature().ok()?;
    item.set_stroke_points(&POINTS).ok()?;
    item.set_armature_parents(&PARENTS).ok()?;
    item.set_op(Op::Add).ok()?;
    let node = document.add_item(layer, &item).ok()?;
    Some((document, layer, node))
}

#[test]
fn nothing_about_a_placed_armature_can_be_read_back() {
    let Some((document, layer, node)) = authored() else {
        return;
    };

    // Not the parents, which have no reader at all — and not the positions
    // and radii either. `clay_layer_stroke_points` is the readback for the
    // setter an armature shares, and it refuses the primitive outright:
    //
    //   curve points need CLAY_PRIM_STROKE or CLAY_PRIM_SWEPT
    //
    // So an armature is write-only in both halves. A host that reopens a
    // document has the skinned surface and nothing else.
    let refusal = document
        .stroke_points(layer, node)
        .expect_err("an armature answered; recover the tree instead of this");
    let detail = format!("{refusal}");
    assert!(
        detail.contains("CLAY_PRIM_STROKE"),
        "the refusal changed shape: {detail}"
    );
}

#[test]
fn a_stroke_on_the_same_call_answers_normally() {
    // The contrast that makes the gap a gap rather than a design: the same
    // call, on the primitive it was written for, round trips exactly as its
    // documentation promises.
    let Some((mut document, layer, _)) = authored() else {
        return;
    };
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
fn the_abi_has_no_armature_reader_at_all() {
    // Stated as a grep over the header so it fails when one is added, rather
    // than sitting in a comment nobody rechecks. Both halves count: a reader
    // for the parents, or `clay_layer_stroke_points` learning the primitive.
    let header = include_str!("../../../vendor/ClayCore/bindings/c/clay.h");
    let readers: Vec<&str> = header
        .lines()
        .filter(|line| line.contains("armature") && line.contains("out_"))
        .collect();
    assert!(
        readers.is_empty(),
        "the ABI grew an armature reader: {readers:?} — recover the tree on \
         load instead of leaving this note"
    );
}
