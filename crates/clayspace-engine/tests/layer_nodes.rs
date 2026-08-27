//! Walking a layer's nodes, as ClayCore 0.30.0 made possible (#91).
//!
//! Finding a reloaded rig used to mean probing node ids upward and giving up
//! after a run of misses. That is a guess about how long a gap can be, and ids
//! are not dense: a removal leaves one, and nothing bounds how long it is. The
//! probe therefore lost every node past the longest run it happened to
//! tolerate — sixteen, here — and no value of "long enough" was defensible.
//!
//! `a_rig_is_found_past_a_long_gap_in_the_ids` is the one that matters: it is
//! the case the old probe got wrong, and it is written so that it fails again
//! if anyone reintroduces a bounded scan.

use clayspace_engine::claycore::{Document, Item, LayerId};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{ArmatureModel, DocumentModel};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy).expect("a document")
}

fn scratch(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("clayspace-nodes-{name}.clayspace"));
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn an_empty_layer_enumerates_to_nothing() {
    let mut doc = Document::new().expect("a document");
    let layer = doc.add_sdf_layer("L").expect("an sdf layer");
    assert_eq!(doc.layer_node_count(layer).expect("a count"), 0);
    assert!(doc.layer_nodes(layer).expect("an enumeration").is_empty());
}

#[test]
fn nodes_come_back_in_evaluation_order() {
    // Index 0 is evaluated first, which is the order the stack is placed
    // against — so an enumeration that reordered them would be worse than
    // none.
    let mut doc = Document::new().expect("a document");
    let layer = doc.add_sdf_layer("L").expect("an sdf layer");
    let mut placed = Vec::new();
    for offset in 0..4 {
        let mut item = Item::sphere(0.2).expect("a sphere");
        item.set_position([offset as f32 * 0.5, 0.0, 0.0])
            .expect("a position");
        placed.push(doc.add_item(layer, &item).expect("place it"));
    }

    assert_eq!(doc.layer_node_count(layer).expect("a count"), placed.len());
    assert_eq!(
        doc.layer_nodes(layer).expect("an enumeration"),
        placed,
        "the nodes came back in a different order than they were placed"
    );
}

#[test]
fn an_index_past_the_end_is_refused_rather_than_wrapped() {
    // What lets a host walk to the end without a sentinel.
    let mut doc = Document::new().expect("a document");
    let layer = doc.add_sdf_layer("L").expect("an sdf layer");
    let item = Item::sphere(0.2).expect("a sphere");
    doc.add_item(layer, &item).expect("place it");

    assert!(doc.layer_node_at(layer, 0).is_ok());
    assert!(doc.layer_node_at(layer, 1).is_err());
    assert!(doc.layer_node_at(layer, 9_999).is_err());
}

#[test]
fn a_layer_with_no_sdf_content_counts_zero() {
    // A voxel layer holds a grid rather than nodes. Zero is the honest answer
    // and the one the evaluation entry points give, so it is not an error.
    let mut doc = Document::new().expect("a document");
    let (layer, _) = doc.add_voxel_layer("V", 0.05).expect("a voxel layer");
    assert_eq!(doc.layer_node_count(layer).expect("a count"), 0);
}

#[test]
fn a_rig_is_found_past_a_long_gap_in_the_ids() {
    // The regression, and the case the old probe actually got wrong.
    //
    // Removing a run of items leaves a gap in the id space, and the gap
    // *survives a round trip*: a document authored this way reopens with ids
    // [1, 32..42] rather than renumbered. The old scan started at 1, found
    // node 1, then missed sixteen consecutively and gave up — never reaching
    // the rig at 42, and reporting a document with no armature at all.
    //
    // Built through the engine's own document because the host has no verb for
    // "remove one item": what matters is that a *file* shaped like this loads,
    // however it came to be shaped that way.
    let mut doc = Document::new().expect("a document");
    let layer = doc.add_sdf_layer("Forma").expect("an sdf layer");
    let mut placed = Vec::new();
    for i in 0..40 {
        let mut item = Item::sphere(0.2).expect("a sphere");
        item.set_position([i as f32 * 0.1, 0.0, 0.0])
            .expect("a position");
        placed.push(doc.add_item(layer, &item).expect("place it"));
    }
    for node in &placed[1..31] {
        doc.remove_node(layer, *node).expect("remove it");
    }

    let mut arm = Item::armature().expect("an armature");
    arm.set_stroke_points(&[0.0, 0.0, 0.0, 0.3, 0.5, 0.0, 0.0, 0.2])
        .expect("its points");
    arm.set_armature_parents(&[0, 0]).expect("its parents");
    let rig = doc.add_item(layer, &arm).expect("place the rig");

    // The gap is the point of the test, so it is asserted rather than assumed.
    let ids = doc.layer_nodes(layer).expect("an enumeration");
    let first = ids.first().expect("a first node");
    let widest = ids
        .windows(2)
        .map(|pair| pair[1].get().saturating_sub(pair[0].get()))
        .max()
        .unwrap_or(0);
    assert!(
        widest > 16,
        "this document has no gap wider than a bounded probe would tolerate \
         ({widest}), so it cannot show the difference. ids: {ids:?}"
    );
    let _ = (first, rig);

    let path = scratch("gap");
    doc.save(&path).expect("save");

    let mut host = document();
    host.open(&path).expect("open");
    let tree = host
        .armature()
        .expect("the rig was not found past the gap in the id space");
    assert_eq!(tree.nodes.len(), 2, "the rig came back a different size");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn enumeration_finds_the_same_rig_a_probe_would_on_a_clean_document() {
    // The ordinary case, so the replacement is not only better in the corner.
    let mut document = document();
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    document
        .add_zsphere(0, [0.5, 0.0, 0.0], 0.2, false)
        .expect("shoulder");

    let path = scratch("clean");
    document.save(&path).expect("save");
    let mut reopened = document;
    reopened.open(&path).expect("open");

    assert_eq!(
        reopened.armature().expect("a rig").nodes.len(),
        2,
        "the rig did not come back on a document with no gaps at all"
    );
    let _ = std::fs::remove_file(&path);
}

/// Kept so the layer type is visible to readers of the imports.
const _: fn(LayerId) -> LayerId = |l| l;
