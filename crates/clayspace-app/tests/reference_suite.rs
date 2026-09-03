//! The suite the figures are measured on, checked for being what it says.
//!
//! A benchmark's baseline is only comparable if the scene behind it is the one
//! the baseline was recorded on. The revision in `Conditions::scenes` is what
//! declares that, and a declaration nobody checks is how a member comes to
//! build something else while still calling itself `r1` — every figure under
//! it then moves for a reason no diff explains.
//!
//! ```sh
//! cargo test -p clayspace-app --release --test reference_suite -- --nocapture
//! ```

use clayspace_app::Scene;
use clayspace_engine::BackendPolicy;
use clayspace_model::{Representation, SculptModel};

/// How far a member may drift from its recorded size before it is a different
/// scene.
///
/// Not zero: a marching cubes surface and a rasterized grid both depend on the
/// engine's own sampling, and an engine release moves them by a little without
/// the scene having changed at all. Ten percent catches a member that stopped
/// building what it says and lets a release through.
const TOLERANCE: f64 = 0.10;

fn policy() -> Option<BackendPolicy> {
    BackendPolicy::discover(None).ok()
}

#[test]
fn every_member_builds_the_size_it_says() {
    let Some(policy) = policy() else {
        return;
    };
    let mut wrong = Vec::new();
    for scene in Scene::ALL {
        let Ok(mut document) = scene.build(policy.clone()) else {
            panic!("{} would not build", scene.member());
        };
        let Some(size) = scene.size(&mut document) else {
            panic!("{} cannot say how big it is", scene.member());
        };
        let (expected, unit) = scene.expected_size();
        let drift = (size as f64 - expected as f64).abs() / (expected.max(1) as f64);
        println!(
            "{:<16} {:>8} {unit:<16} recorded {expected:>8}  ({:+.1}%)",
            scene.member(),
            size,
            drift * 100.0 * (size as f64 - expected as f64).signum()
        );
        if drift > TOLERANCE {
            wrong.push(format!(
                "{} built {size} {unit} against a recorded {expected}",
                scene.member()
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "the suite is not the shape its revisions claim:\n  {}\n\
         Either the change to the scene was not meant, or its revision needs \
         bumping and its baselines re-recording.",
        wrong.join("\n  ")
    );
}

/// Every representation that has a member is measured on its own, and the one
/// that has none says so rather than borrowing somebody else's.
///
/// The second half is the half worth having. A representation with no member is
/// a family of figures nobody is taking, and the failure that matters is not
/// "it has no member" — that is a deliberate state, stated in
/// `Scene::for_representation` and reported every run as
/// `Skip::NoReferenceScene` — but a member arriving that builds the wrong
/// subject, which is what the equality below catches.
#[test]
fn every_member_measures_the_representation_it_claims() {
    let mut unmeasured = Vec::new();
    for representation in Representation::ALL {
        match Scene::for_representation(representation) {
            Some(scene) => assert_eq!(
                scene.representation(),
                representation,
                "{representation:?} is measured on a scene of another representation"
            ),
            None => unmeasured.push(representation),
        }
    }
    assert_eq!(
        unmeasured,
        vec![Representation::Multires],
        "the set of representations with no reference member has changed; if \
         one gained a member, its baselines have to be recorded, and if one \
         lost its member that is the silence this file exists to catch"
    );
}

#[test]
fn a_probe_lands_on_the_subject() {
    let Some(policy) = policy() else {
        return;
    };
    for scene in Scene::ALL {
        let Ok(document) = scene.build(policy.clone()) else {
            continue;
        };
        assert!(
            scene.probe(&document).is_some(),
            "{} has nowhere to land a probe edit",
            scene.member()
        );
    }
}

#[test]
fn a_members_active_layer_is_the_representation_it_claims() {
    let Some(policy) = policy() else {
        return;
    };
    for scene in Scene::ALL {
        let Ok(document) = scene.build(policy.clone()) else {
            continue;
        };
        assert_eq!(
            document.active_representation(),
            scene.representation(),
            "{} builds a document whose active layer is something else",
            scene.member()
        );
    }
}
