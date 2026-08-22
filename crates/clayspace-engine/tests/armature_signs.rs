//! Negative ZSpheres, as ClayCore 0.30.0 made them expressible (#99).
//!
//! Three things were wrong while a negative had to be a separate subtractive
//! item, and each gets a test here because each was visible to a sculptor:
//!
//! - the **membrane** along a negative sphere's links was still drawn, so the
//!   cut sat inside a skin that had not been cut;
//! - the **sign was lost on reload**, because the reader saw an armature of
//!   positives and some unrelated spheres beside it;
//! - **only a leaf** could be negative, since anything hanging off one would
//!   have been orphaned when the node left the tree.
//!
//! The engine now builds the positive armature minus the negative one, with a
//! link only between two nodes of the same sign.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{ArmatureModel, DocumentModel, SculptModel};

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy).ok()
}

/// Whether the surface is solid at a point, probed straight down the z axis.
fn solid_at(document: &ClayDocument, at: [f32; 3]) -> bool {
    let origin = [at[0], at[1], at[2] + 4.0];
    document
        .pick(origin, [0.0, 0.0, -1.0])
        .map(|hit| hit[2] > at[2] - 0.5)
        .unwrap_or(false)
}

fn scratch(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("clayspace-signs-{name}.clayspace"));
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn a_negative_sphere_may_carry_a_limb() {
    // The rule that went with #99. It was never ZBrush's — it was the old
    // ABI's, and it is why `set_negative` used to refuse anything with a
    // child.
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    let socket = document
        .add_zsphere(0, [0.5, 0.0, 0.0], 0.2, false)
        .expect("socket");
    let tip = document
        .add_zsphere(socket, [1.0, 0.0, 0.0], 0.15, false)
        .expect("tip");

    document
        .set_zsphere_negative(socket, true)
        .expect("a sphere with a child can cut");

    let tree = document.armature().expect("a tree");
    assert!(tree.get(socket).expect("the socket").negative);
    assert!(
        !tree.get(tip).expect("the tip").negative,
        "the child was dragged negative with its parent"
    );
}

#[test]
fn the_sign_survives_a_round_trip() {
    // The loss that made this the same shape of bug as a rename: a reopened
    // document reported every sphere positive, so the indentation was in the
    // surface and could never be undone.
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    let shoulder = document
        .add_zsphere(0, [0.5, 0.0, 0.0], 0.2, false)
        .expect("shoulder");
    let socket = document
        .add_zsphere(shoulder, [1.0, 0.0, 0.0], 0.15, false)
        .expect("socket");
    document
        .set_zsphere_negative(socket, true)
        .expect("a negative sphere");

    let path = scratch("roundtrip");
    document.save(&path).expect("save");

    let mut reopened = document;
    reopened.open(&path).expect("open");

    let tree = reopened.armature().expect("a reopened tree");
    assert_eq!(tree.nodes.len(), 3, "the rig came back a different size");
    assert!(
        tree.get(socket).expect("the socket").negative,
        "the sign was lost on reload — a reopened rig reports all-positive"
    );
    assert!(
        !tree.get(shoulder).expect("the shoulder").negative,
        "a positive sphere came back negative"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_rig_saved_without_signs_reads_back_positive() {
    // The compatibility direction, which the engine pads rather than refuses:
    // a rig with no negatives stores a short sign array or none at all, and
    // has to come back all-positive rather than failing to load.
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    document
        .add_zsphere(0, [0.5, 0.0, 0.0], 0.2, false)
        .expect("shoulder");

    let path = scratch("unsigned");
    document.save(&path).expect("save");
    let mut reopened = document;
    reopened.open(&path).expect("open");

    let tree = reopened.armature().expect("a reopened tree");
    assert!(
        tree.nodes.iter().all(|node| !node.negative),
        "a rig with no negatives came back carrying one"
    );
}

#[test]
fn the_membrane_along_a_negative_link_is_not_drawn() {
    // The visible half, and the one a separate subtractive sphere could never
    // fix: the cutter carved a ball, and the skin between the negative sphere
    // and its parent went on being drawn straight through it.
    //
    // Probed rather than asserted about triangles: the midpoint of the link is
    // where the membrane would be, and it is far enough from both spheres that
    // neither one's own body reaches it.
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.4).expect("root");
    let tip = document
        .add_zsphere(0, [1.2, 0.0, 0.0], 0.12, false)
        .expect("tip");

    let midpoint = [0.6, 0.0, 0.0];
    assert!(
        solid_at(&document, midpoint),
        "the membrane was not there to begin with, so this proves nothing"
    );

    document
        .set_zsphere_negative(tip, true)
        .expect("a negative sphere");

    assert!(
        !solid_at(&document, midpoint),
        "the membrane along a negative sphere's link is still drawn — the \
         cut sits inside a skin that was not cut"
    );
    // And the root it hangs off is untouched: a carve must not sweep a
    // positive parent's radius, or an eye socket swallows the head.
    assert!(
        solid_at(&document, [0.0, 0.0, 0.0]),
        "the carve swallowed the sphere it was cutting into"
    );
}

#[test]
fn turning_a_sphere_back_positive_restores_its_membrane() {
    // The inverse, so the sign is a property being toggled rather than a
    // one-way demolition.
    let Some(mut document) = document() else {
        return;
    };
    document.begin_armature([0.0, 0.0, 0.0], 0.4).expect("root");
    let tip = document
        .add_zsphere(0, [1.2, 0.0, 0.0], 0.12, false)
        .expect("tip");
    let midpoint = [0.6, 0.0, 0.0];

    document.set_zsphere_negative(tip, true).expect("negative");
    assert!(!solid_at(&document, midpoint));

    document
        .set_zsphere_negative(tip, false)
        .expect("positive again");
    assert!(
        solid_at(&document, midpoint),
        "the membrane did not come back when the sphere did"
    );
}
