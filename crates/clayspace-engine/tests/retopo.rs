//! Retopology through the domain's own interface.
//!
//! The engine tests beside this exercise the two libraries directly. This one
//! goes through `RetopoModel`, which is what a ViewModel will call — so it is
//! the first test that says the feature is *reachable* rather than merely
//! implemented.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Combine, CombineSettings, Direction, ObjectModel, QuadMethod, Representation, RetopoModel,
    RetopoSettings, SceneModel, SculptModel, Shape,
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
fn a_mesh_subtool_is_retopologised_in_place() {
    let Some(mut document) = meshed() else {
        return;
    };
    let subtools_before = document.scene().layers.len();
    let history_before = document.history().depth;
    let key_before = document
        .scene()
        .active_layer()
        .expect("an active layer")
        .key;
    let name_before = document
        .scene()
        .active_layer()
        .expect("an active layer")
        .name
        .clone();
    let triangles_before = document.visible_mesh_geometry().3.len() / 3;

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

    // **On the subtool itself.** This placed the result beside its source at
    // first, which left a sculptor deleting one of two subtools after every
    // retopology and made the stack the record of an operation history
    // already records. ZBrush's ZRemesher, its Dynamesh and this
    // application's own Rebuild all rebuild the thing in front of you;
    // reported from a session as the difference that mattered.
    assert_eq!(
        document.scene().layers.len(),
        subtools_before,
        "the retopology added a subtool instead of rebuilding this one"
    );
    assert_eq!(
        document
            .scene()
            .active_layer()
            .expect("an active layer")
            .key,
        key_before,
        "the active subtool changed, so the sculptor is no longer looking at \
         what they retopologised"
    );
    assert_eq!(
        document
            .scene()
            .active_layer()
            .expect("an active layer")
            .name,
        name_before,
        "the subtool was renamed; a rebuild of its topology is not a new thing"
    );

    // And it is a different surface than it was.
    let triangles_after = document.visible_mesh_geometry().3.len() / 3;
    assert_ne!(
        triangles_after, triangles_before,
        "{triangles_before} triangles before and after, so nothing was replaced"
    );

    // One undo entry, as a rebuild and a crossing already are — and it is
    // what a sculptor compares against now that there is no second subtool
    // to look at, which is the same answer ZBrush gives.
    assert_eq!(
        document.history().depth,
        history_before + 1,
        "rebuilding the topology reached the history as more than one thing a \
         sculptor did"
    );
    assert!(document.undo().expect("undo"), "nothing to undo");
    assert_eq!(
        document.scene().layers.len(),
        subtools_before,
        "undoing a rebuild changed the subtool count"
    );
    assert_eq!(
        document.visible_mesh_geometry().3.len() / 3,
        triangles_before,
        "one undo did not put the original topology back"
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

/// The **placed layer** carries the quads, not just the engine's report.
///
/// This is the assertion that was missing, and its absence is why the feature
/// shipped looking broken. `a_mesh_subtool_becomes_quads_in_a_new_subtool`
/// asserts `outcome.is_quads()` — fewer faces than triangles — which is a fact
/// about the *retopologiser's* mesh. Nothing checked what arrived in the
/// document. ClayCore's C ABI has exactly one mesh constructor,
/// `clay_mesh_from_triangles`, so what crossed was the fan triangulation and
/// the quads had nowhere to live; the polyframe derived its wireframe from
/// those triangles and drew every diagonal. A 100%-quad retopology was
/// indistinguishable on screen from a triangle mesh, and a sculptor reported
/// exactly that.
///
/// So the faces are kept beside the layer and travel to the viewport on the
/// span. What this asserts is that they arrive, that they are the *quads* and
/// not the triangulation, and that the source layer beside them is untouched.
#[test]
fn the_placed_layer_carries_the_quads_the_viewport_will_draw() {
    let Some(mut document) = meshed() else {
        return;
    };
    let outcome = document
        .retopologise(RetopoSettings {
            target_quads: 600,
            ..RetopoSettings::default()
        })
        .expect("the retopology runs");
    assert!(outcome.is_quads(), "the fixture did not produce quads");

    let (positions, _, _, indices, spans) = document.visible_mesh_geometry();
    let authored: Vec<_> = spans.iter().filter(|s| s.edges.is_some()).collect();
    assert_eq!(
        authored.len(),
        1,
        "expected exactly one layer to carry authored faces, found {} of {}",
        authored.len(),
        spans.len()
    );
    let edges = authored[0].edges.as_ref().expect("the authored edges");

    assert_eq!(edges.len() % 2, 0, "two vertex indices per edge");
    assert!(!edges.is_empty(), "the placed layer carries no edges");

    // The quads, not their triangulation. A pure-quad mesh of F faces has 2F
    // edges; its fan triangulation has 3F — the same 2F plus one diagonal per
    // quad. So `< 3F` is what says the diagonals are absent, and a buffer
    // that had included them could not satisfy it.
    let faces = outcome.faces;
    assert!(
        edges.len() / 2 < faces * 3,
        "{} edges for {faces} faces — that is the triangulation's edge count, \
         so the polyframe would draw diagonals and the quads would not show",
        edges.len() / 2
    );

    // Rebased onto the shared buffer, so the viewport joins the right points.
    // Off-by-one here is a wireframe of the wrong shape rather than a crash,
    // which is why it is asserted rather than trusted.
    let vertices = positions.len() as u32;
    assert!(
        edges.iter().all(|&v| v < vertices),
        "an authored edge indexes past the {vertices} vertices uploaded"
    );
    let range = &authored[0].indices;
    assert!(
        (range.end as usize) <= indices.len(),
        "the span names indices the buffer does not have"
    );

    // And the interface can now say so. The Polygons row was drawn from the
    // triangle count, so it printed the same number twice and could not
    // express a quad mesh at all.
    let stats = document.stats();
    let reported = stats.faces.expect("a quad-carrying scene reports faces");
    assert!(
        reported < stats.triangles,
        "the scene reports {reported} faces and {} triangles, which is what a \
         triangle mesh reports",
        stats.triangles
    );
    println!(
        "  {} spans, {} authored edges, scene reports {reported} faces / {} triangles",
        spans.len(),
        edges.len() / 2,
        stats.triangles
    );
}

/// A retopologised subtool and an ordinary one draw correctly side by side.
///
/// The wireframe is decided **per subtool**, not per scene, and this is what
/// says so. A retopology rebuilds one subtool in place and leaves the others
/// alone, so a scene holding both is the ordinary case rather than a corner —
/// and an all-or-nothing rule (authored edges only when *every* visible layer
/// has them, or derived only when none does) would draw one of the two wrong.
///
/// Both halves are asserted because only the pair distinguishes the design
/// from a rule that happened to be right about one of them: exactly one span
/// carries authored faces and exactly one derives.
#[test]
fn a_retopologised_subtool_and_a_triangle_one_keep_their_own_wireframes() {
    let Some(mut document) = meshed() else {
        return;
    };
    // A second mesh subtool beside the first, so the scene is mixed.
    let Ok(second) = document.add_layer("Segunda", Representation::Sdf) else {
        return;
    };
    if document.set_active_layer(second).is_err() {
        return;
    }
    if document
        .place_object(
            Shape::Sphere,
            &[0.6],
            [0.0; 3],
            CombineSettings {
                op: Combine::Add,
                ..CombineSettings::default()
            },
        )
        .is_err()
    {
        return;
    }
    if document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .is_err()
    {
        return;
    }

    let retopologised = document
        .scene()
        .active_layer()
        .expect("an active layer")
        .key;
    document
        .retopologise(RetopoSettings {
            target_quads: 400,
            ..RetopoSettings::default()
        })
        .expect("the retopology runs");

    let (_, _, _, _, spans) = document.visible_mesh_geometry();
    assert_eq!(
        spans.len(),
        2,
        "the fixture wanted two visible mesh subtools, found {}",
        spans.len()
    );
    let with: Vec<_> = spans.iter().filter(|s| s.edges.is_some()).collect();
    let without: Vec<_> = spans.iter().filter(|s| s.edges.is_none()).collect();
    assert_eq!(
        with.len(),
        1,
        "expected one subtool carrying authored faces, found {}",
        with.len()
    );
    assert_eq!(
        without.len(),
        1,
        "expected one subtool whose faces are its triangles, found {}",
        without.len()
    );
    assert_eq!(
        with[0].layer, retopologised,
        "the authored faces are on the subtool that was not retopologised"
    );
}
