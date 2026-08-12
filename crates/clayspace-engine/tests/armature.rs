//! ZSpheres against a real document.
//!
//! The domain tests cover the tree; these cover what reaches the surface. The
//! one that matters most is the puppet rule: a shoulder moves and the arm goes
//! with it, in the geometry and not only in the data structure.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{Armature, ArmatureModel, SceneModel, SculptModel, SkinSettings};

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy).ok()
}

/// Is there surface at this point?
fn solid_at(document: &ClayDocument, at: [f32; 3]) -> bool {
    // A ray straight down at the point: if it stops above the point's own
    // height there is material there.
    let origin = [at[0], at[1], at[2] + 4.0];
    document
        .pick(origin, [0.0, 0.0, -1.0])
        .map(|hit| hit[2] > at[2] - 0.5)
        .unwrap_or(false)
}

/// A shoulder with an arm hanging off it.
fn rig(document: &mut ClayDocument) -> Armature {
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    let shoulder = document
        .add_zsphere(0, [0.5, 0.0, 0.0], 0.2, false)
        .expect("shoulder");
    document
        .add_zsphere(shoulder, [1.0, 0.0, 0.0], 0.15, false)
        .expect("elbow");
    document.armature().expect("a tree")
}

#[test]
fn an_armature_becomes_surface() {
    let Some(mut document) = document() else {
        return;
    };
    assert!(document.armature().is_none(), "a fresh layer has no rig");

    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    let tree = document.armature().expect("a tree");
    assert_eq!(tree.nodes.len(), 1);
    assert!(
        solid_at(&document, [0.0, 0.0, 0.0]),
        "the root sphere produced no surface"
    );
}

#[test]
fn a_child_is_skinned_to_its_parent() {
    // The link, not just the spheres: the point between two ZSpheres has to be
    // solid or they read as beads rather than a limb.
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    document
        .add_zsphere(0, [0.8, 0.0, 0.0], 0.2, false)
        .expect("child");

    assert!(
        solid_at(&document, [0.4, 0.0, 0.0]),
        "the space between two spheres is not skinned"
    );
}

#[test]
fn moving_a_shoulder_moves_the_arm_in_the_geometry() {
    let Some(mut document) = document() else {
        return;
    };
    let tree = rig(&mut document);
    assert_eq!(tree.nodes.len(), 3);
    assert!(solid_at(&document, [1.0, 0.0, 0.0]), "the elbow is there");

    // Lift the shoulder. The elbow hangs off it and must come along.
    document.move_zsphere(1, [0.0, 1.0, 0.0]).expect("move");

    let moved = document.armature().expect("a tree");
    assert_eq!(moved.nodes[1].position[1], 1.0, "the shoulder did not move");
    assert_eq!(moved.nodes[2].position[1], 1.0, "the elbow stayed behind");
    assert_eq!(moved.nodes[0].position[1], 0.0, "the root should not move");

    assert!(
        solid_at(&document, [1.0, 1.0, 0.0]),
        "the arm's surface did not follow the shoulder"
    );
    assert!(
        !solid_at(&document, [1.0, 0.0, 0.0]),
        "the arm left surface behind where it used to be"
    );
}

#[test]
fn removing_a_shoulder_removes_the_arm() {
    let Some(mut document) = document() else {
        return;
    };
    rig(&mut document);
    assert!(solid_at(&document, [1.0, 0.0, 0.0]));

    document.remove_zsphere(1).expect("remove");

    let tree = document.armature().expect("a tree");
    assert_eq!(tree.nodes.len(), 1, "only the root should remain");
    assert!(
        !solid_at(&document, [1.0, 0.0, 0.0]),
        "the arm's surface outlived the arm"
    );
}

#[test]
fn the_root_cannot_be_removed() {
    // Removing the root would leave a tree with no tree in it. Refusing beats
    // silently emptying the layer.
    let Some(mut document) = document() else {
        return;
    };
    rig(&mut document);
    assert!(document.remove_zsphere(0).is_err());
    assert_eq!(document.armature().expect("a tree").nodes.len(), 3);
}

#[test]
fn resizing_a_sphere_changes_what_it_covers() {
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.2).expect("root");
    let probe = [0.0f32, 0.5, 0.0];
    assert!(!solid_at(&document, probe), "0.2 should not reach 0.5");

    document.resize_zsphere(0, 0.7).expect("resize");
    assert!(
        solid_at(&document, probe),
        "the sphere did not grow to cover the probe"
    );
}

#[test]
fn mirrored_authoring_puts_a_sphere_on_both_sides() {
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    document
        .add_zsphere(0, [0.6, 0.0, 0.0], 0.2, true)
        .expect("mirrored child");

    let tree = document.armature().expect("a tree");
    assert_eq!(tree.nodes.len(), 3, "the reflection was not added");
    assert!(
        solid_at(&document, [0.6, 0.0, 0.0]) && solid_at(&document, [-0.6, 0.0, 0.0]),
        "one side of a mirrored pair is missing from the surface"
    );
}

#[test]
fn a_sphere_on_the_mirror_plane_is_added_once() {
    // The engine's rule, and the one that stops a spine growing two of
    // everything.
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    document
        .add_zsphere(0, [0.0, 0.5, 0.0], 0.2, true)
        .expect("child on the plane");
    assert_eq!(document.armature().expect("a tree").nodes.len(), 2);
}

#[test]
fn a_mirrored_child_hangs_off_the_mirrored_parent() {
    // Two arms off two shoulders, not both off one.
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    let right_shoulder = document
        .add_zsphere(0, [0.5, 0.0, 0.0], 0.2, true)
        .expect("shoulders");
    document
        .add_zsphere(right_shoulder, [1.0, 0.0, 0.0], 0.15, true)
        .expect("elbows");

    let tree = document.armature().expect("a tree");
    assert_eq!(tree.nodes.len(), 5, "root, two shoulders, two elbows");

    // The left elbow's parent must be the left shoulder, not the right one.
    let left_elbow = tree
        .nodes
        .iter()
        .position(|n| n.position[0] < -0.9)
        .expect("a left elbow");
    let left_shoulder = tree
        .nodes
        .iter()
        .position(|n| (n.position[0] + 0.5).abs() < 1e-3)
        .expect("a left shoulder");
    assert_eq!(
        tree.nodes[left_elbow].parent as usize, left_shoulder,
        "the mirrored elbow hangs off the wrong shoulder"
    );
}

#[test]
fn reparenting_moves_a_limb_to_another_joint() {
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    document
        .add_zsphere(0, [0.6, 0.0, 0.0], 0.2, false)
        .expect("one");
    let second = document
        .add_zsphere(0, [-0.6, 0.0, 0.0], 0.2, false)
        .expect("two");

    document.reparent_zsphere(second, 1).expect("reparent");
    assert_eq!(document.armature().expect("a tree").nodes[2].parent, 1);

    // And a cycle is refused rather than accepted into a tree the engine
    // would reject anyway.
    assert!(document.reparent_zsphere(1, 2).is_err());
}

#[test]
fn the_skin_setting_reaches_the_surface() {
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    document
        .add_zsphere(0, [0.8, 0.0, 0.0], 0.2, false)
        .expect("child");

    // A point just outside the thin rig and inside the thick one.
    let probe = [0.0f32, 0.45, 0.0];
    document
        .set_skin(SkinSettings { thickness: 1.0 })
        .expect("thin");
    assert!(!solid_at(&document, probe), "0.3 should not reach 0.45");

    document
        .set_skin(SkinSettings { thickness: 2.0 })
        .expect("thick");
    assert!(
        solid_at(&document, probe),
        "raising the skin thickness did not thicken the rig"
    );
    assert_eq!(document.skin().thickness, 2.0);

    // And the authored tree is untouched, so it is reversible.
    assert_eq!(document.armature().expect("a tree").nodes[0].radius, 0.3);
}

#[test]
fn an_armature_edit_without_an_armature_says_so() {
    let Some(mut document) = document() else {
        return;
    };
    let error = document
        .move_zsphere(0, [0.0, 1.0, 0.0])
        .expect_err("moving nothing succeeded");
    assert!(format!("{error}").chars().any(|c| c.is_alphabetic()));
}

#[test]
fn an_armature_belongs_to_the_layer_it_was_authored_on() {
    // Switching layers must not hand the next click someone else's rig.
    let Some(mut document) = document() else {
        return;
    };
    rig(&mut document);
    assert!(document.armature().is_some());

    let other = document
        .add_layer("Outra", clayspace_model::Representation::Sdf)
        .expect("layer");
    document.set_active_layer(other).expect("switch");
    assert!(
        document.armature().is_none(),
        "another layer's rig showed up on this one"
    );
}
