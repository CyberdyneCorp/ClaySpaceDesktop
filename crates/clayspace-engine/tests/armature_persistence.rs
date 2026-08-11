//! Task 6.6: what survives saving a rig and opening it again.
//!
//! Two different things could be meant by "the armature persisted": the
//! *surface* it produced, and the *tree* that produced it. The first is the
//! engine's business and the second is ours, because the parent array cannot
//! be read back out of the ABI. These tests state which is which rather than
//! asserting the happier of the two and calling it done.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{ArmatureModel, DocumentModel, SculptModel};

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy).ok()
}

fn solid_at(document: &ClayDocument, at: [f32; 3]) -> bool {
    let origin = [at[0], at[1], at[2] + 4.0];
    document
        .pick(origin, [0.0, 0.0, -1.0])
        .map(|hit| hit[2] > at[2] - 0.5)
        .unwrap_or(false)
}

/// A root with an arm, saved to a temporary file.
fn saved(document: &mut ClayDocument, path: &std::path::Path) {
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    let shoulder = document
        .add_zsphere(0, [0.5, 0.0, 0.0], 0.2, false)
        .expect("shoulder");
    document
        .add_zsphere(shoulder, [1.0, 0.0, 0.0], 0.15, false)
        .expect("elbow");
    document.save(path).expect("save");
}

fn scratch(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("clayspace-armature-{name}.clayspace"));
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn the_skinned_surface_survives_a_round_trip() {
    // The part that matters to whoever opens the file: the model looks the
    // same. This is the engine's own node, saved and loaded by the engine.
    let Some(mut document) = document() else {
        return;
    };
    let path = scratch("surface");
    saved(&mut document, &path);
    assert!(solid_at(&document, [1.0, 0.0, 0.0]), "the arm was there");

    let mut reopened = document;
    reopened.open(&path).expect("open");

    assert!(
        solid_at(&reopened, [1.0, 0.0, 0.0]),
        "the arm did not survive the round trip"
    );
    assert!(
        solid_at(&reopened, [0.25, 0.0, 0.0]),
        "the skin between the spheres did not survive"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_tree_does_not_survive_and_the_document_says_so() {
    // The honest half. `clay_layer_stroke_points` reads positions and radii
    // back, but there is no reader for the parent array, so the topology a
    // rig *is* cannot be recovered from the file. Rather than invent a
    // plausible tree — nearest-preceding-sphere chaining would produce one,
    // and it would be wrong for any rig that branches — a reopened document
    // reports no armature, and the surface stands on its own.
    //
    // This test is written to fail the day the ABI grows a reader. That is
    // the signal to recover the tree here instead of documenting its loss.
    let Some(mut document) = document() else {
        return;
    };
    let path = scratch("tree");
    saved(&mut document, &path);
    assert_eq!(document.armature().expect("a tree").nodes.len(), 3);

    let mut reopened = document;
    reopened.open(&path).expect("open");

    assert!(
        reopened.armature().is_none(),
        "a tree came back; recover it properly rather than leaving this note"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn rigging_a_reopened_document_starts_a_new_tree_rather_than_editing_a_ghost() {
    // The failure this prevents: a rig authored after a reload silently
    // editing indices into a tree the host no longer has. Beginning a rig
    // replaces whatever the layer held, which is a visible, undoable act.
    let Some(mut document) = document() else {
        return;
    };
    let path = scratch("reauthor");
    saved(&mut document, &path);
    let mut reopened = document;
    reopened.open(&path).expect("open");

    assert!(reopened.move_zsphere(1, [0.0, 1.0, 0.0]).is_err());

    reopened
        .begin_armature([0.0, 1.5, 0.0], 0.3)
        .expect("a fresh rig");
    let tree = reopened.armature().expect("a tree");
    assert_eq!(tree.nodes.len(), 1);
    assert!(solid_at(&reopened, [0.0, 1.5, 0.0]));
    let _ = std::fs::remove_file(&path);
}
