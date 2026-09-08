//! Retopology through the domain's own interface.
//!
//! The engine tests beside this exercise the two libraries directly. This one
//! goes through `RetopoModel`, which is what a ViewModel will call — so it is
//! the first test that says the feature is *reachable* rather than merely
//! implemented.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Direction, QuadMethod, Representation, RetopoModel, RetopoSettings, SceneModel, SculptModel,
};

/// A sculpt that has become a mesh: the starting form, crossed.
fn meshed() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    document.convert_layer(Direction::SdfToMesh, 0.05, 0).ok()?;
    Some(document)
}

#[test]
fn a_mesh_subtool_becomes_quads_in_a_new_subtool() {
    let Some(mut document) = meshed() else {
        return;
    };
    let subtools_before = document.scene().layers.len();
    let history_before = document.history().depth;

    document
        .can_retopologise()
        .expect("a crossed mesh subtool can be retopologised");
    let outcome = document
        .retopologise(RetopoSettings {
            target_quads: 600,
            ..RetopoSettings::default()
        })
        .expect("the retopology runs");

    println!(
        "{} triangles -> {} faces / {} triangles, {:.0}% quads",
        outcome.triangles_before,
        outcome.faces,
        outcome.triangles,
        outcome.quad_share() * 100.0
    );

    // **What says these are quads.** The engine carries quads beside its own
    // triangulation rather than instead of it, so a quad mesh reports fewer
    // faces than triangles where a triangle mesh reports exactly as many.
    assert!(
        outcome.is_quads(),
        "the result has {} faces and {} triangles, which is what a triangle \
         mesh looks like",
        outcome.faces,
        outcome.triangles
    );
    assert!(outcome.triangles_before > 0);

    // Beside the source, not instead of it.
    assert_eq!(
        document.scene().layers.len(),
        subtools_before + 1,
        "the retopology did not arrive as a new subtool"
    );

    // One undo entry, as a crossing and a boolean already are.
    assert_eq!(
        document.history().depth,
        history_before + 1,
        "making the layer and filling it reached the history as more than one \
         thing a sculptor did"
    );
    assert!(document.undo().expect("undo"), "nothing to undo");
    assert_eq!(
        document.scene().layers.len(),
        subtools_before,
        "one undo did not take the retopologised subtool back"
    );
}

/// A field subtool is refused by name rather than crossed for the sculptor.
///
/// Crossing one silently would be a representation change with its own cost
/// and its own undo entry, and a tool that says it rebuilds topology must not
/// perform one.
#[test]
fn a_field_subtool_is_refused_and_the_reason_names_the_representation() {
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form) else {
        return;
    };
    assert_eq!(document.active_representation(), Representation::Sdf);
    let refusal = document
        .can_retopologise()
        .expect_err("a field is not a mesh");
    assert!(
        refusal.contains("Sdf"),
        "the refusal does not say which representation is in the way: {refusal}"
    );
}

/// Every method the domain offers is one the engine accepts.
///
/// Walked rather than spot-checked: the list is the domain's and the
/// enumerants are the engine's, so a method added without a value behind it
/// fails on the row that was added rather than the first time a sculptor
/// picks it.
#[test]
fn every_quad_method_the_domain_offers_runs() {
    for method in QuadMethod::ALL {
        let Some(mut document) = meshed() else {
            return;
        };
        let outcome = document
            .retopologise(RetopoSettings {
                target_quads: 300,
                method,
                ..RetopoSettings::default()
            })
            .unwrap_or_else(|e| panic!("{} was offered and refused: {e}", method.label()));
        println!(
            "  {:<18} {} faces, {} triangles",
            method.label(),
            outcome.faces,
            outcome.triangles
        );
        assert!(outcome.faces > 0);
    }
}
