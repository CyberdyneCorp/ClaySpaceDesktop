//! Task 6.6: what survives saving a rig and opening it again.
//!
//! Two different things could be meant by "the armature persisted": the
//! *surface* it produced, and the *rig* that produced it. Both survive since
//! ClayCore 0.29.0 (#77) — before it a placed armature was write-only, so a
//! reopened document held a skinned shape nobody could pose again.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{ArmatureModel, DocumentModel, SculptModel};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy).expect("a document")
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
    let mut document = document();
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
fn the_tree_comes_back_with_its_topology() {
    // Positions are the easy half. The parent array is what makes a reloaded
    // rig posable, and what no amount of nearest-sphere guessing recovers.
    let mut document = document();
    let path = scratch("tree");
    saved(&mut document, &path);
    let before = document.armature().expect("a tree");
    assert_eq!(before.nodes.len(), 3);

    let mut reopened = document;
    reopened.open(&path).expect("open");

    let after = reopened.armature().expect("the rig came back");
    assert_eq!(after.nodes.len(), before.nodes.len());
    for (index, (was, now)) in before.nodes.iter().zip(after.nodes.iter()).enumerate() {
        assert_eq!(now.parent, was.parent, "node {index} lost its parent");
        for axis in 0..3 {
            assert!(
                (now.position[axis] - was.position[axis]).abs() < 1e-4,
                "node {index} moved on {axis}: {:?} against {:?}",
                now.position,
                was.position
            );
        }
        assert!(
            (now.radius - was.radius).abs() < 1e-4,
            "node {index} changed radius: {} against {}",
            now.radius,
            was.radius
        );
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_reopened_rig_can_be_posed() {
    // The point of recovering the tree at all: moving a shoulder after a
    // reload has to carry the arm, exactly as it did before the save.
    let mut document = document();
    let path = scratch("reauthor");
    saved(&mut document, &path);
    let mut reopened = document;
    reopened.open(&path).expect("open");

    reopened.move_zsphere(1, [0.0, 1.0, 0.0]).expect("pose it");

    let tree = reopened.armature().expect("a tree");
    assert_eq!(tree.nodes[1].position[1], 1.0, "the shoulder did not move");
    assert_eq!(tree.nodes[2].position[1], 1.0, "the elbow stayed behind");
    assert!(
        solid_at(&reopened, [1.0, 1.0, 0.0]),
        "the arm's surface did not follow the shoulder after a reload"
    );
    let _ = std::fs::remove_file(&path);
}
