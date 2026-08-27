//! The whole-form deformers, and what one press of undo takes back.
//!
//! A deformer states something about the *form* rather than about a dab, which
//! is why it reaches the document through `apply_operation` rather than through
//! a stroke. It moves every vertex it touches in one call, so "one undo step"
//! is a real question rather than an obvious one: a mesh layer carries no
//! engine history at all, and what undo takes back is the delta record the
//! adapter keeps beside it.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    DeformSettings, DeformVerb, ExchangeModel, ExportSettings, ImportSettings, LayerOperation,
    Representation, SceneModel, SculptModel,
};

/// Ignored on macOS, and that is the point of the change that added it.
///
/// These tests used to disable themselves: `with_mesh` returned `Option` and
/// every step ended in `?`, so a failed export made the test return early and
/// the run go green. Turning those into `expect` showed that on the Metal
/// backend the export comes back with no triangles at all — #37 — and that
/// these tests had never run there.
///
/// `ignore` is over-broad, because `macOS, CPU only` can run them and Rust
/// cannot `cfg` on a runtime backend. It is still the right trade: an ignored
/// test is *reported* as ignored, where the old shape was reported as passing.
/// Remove the attribute when #37 closes.
///
/// A document whose active layer is a mesh, made by exporting the starting form
/// and importing it back — the only route a mesh layer has into a document.
fn with_mesh(who: &str) -> (ClayDocument, std::path::PathBuf) {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let path = std::env::temp_dir().join(format!("clayspace-deform-{who}.obj"));
    let _ = std::fs::remove_file(&path);
    document
        .export_mesh(&path, ExportSettings::default())
        .expect("export a mesh");
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import it back");
    let key = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the imported mesh is a layer");
    document.set_active_layer(key).expect("activate the mesh");
    (document, path)
}

/// Every vertex of the mesh layers, so a deformation can be compared exactly.
fn vertices(document: &mut ClayDocument) -> Vec<[f32; 3]> {
    document.visible_mesh_geometry().0
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "the Metal export yields no triangles; see #37"
)]
fn a_taper_moves_the_form_and_one_undo_takes_it_back() {
    let (mut document, path) = with_mesh("taper");
    let before = vertices(&mut document);
    assert!(!before.is_empty(), "the fixture carries no vertices");

    let settings = DeformSettings {
        verb: DeformVerb::Taper,
        axis: [0.0, 1.0, 0.0],
        span: 2.0,
        scale_start: 1.0,
        scale_end: 0.3,
        ..Default::default()
    };
    let outcome = document
        .apply_operation(settings.operation())
        .expect("a taper on a mesh layer");
    assert!(outcome.changed, "the taper moved nothing");

    let after = vertices(&mut document);
    assert_ne!(before, after, "the taper left every vertex where it was");

    // One press. The deformer touched thousands of vertices in one call and a
    // sculptor did one thing, so one undo is what takes it back — not one per
    // vertex and not none at all.
    assert!(
        document.undo().expect("undo"),
        "undo found nothing to take back"
    );
    assert_eq!(
        vertices(&mut document),
        before,
        "one undo did not restore the form the taper started from"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "the Metal export yields no triangles; see #37"
)]
fn a_twist_moves_the_form_and_one_undo_takes_it_back() {
    let (mut document, path) = with_mesh("twist");
    let before = vertices(&mut document);

    let settings = DeformSettings {
        verb: DeformVerb::Twist,
        axis: [0.0, 1.0, 0.0],
        span: 2.0,
        degrees: 90.0,
        ..Default::default()
    };
    let outcome = document
        .apply_operation(settings.operation())
        .expect("a twist on a mesh layer");
    assert!(outcome.changed, "the twist moved nothing");
    assert_ne!(vertices(&mut document), before);

    assert!(document.undo().expect("undo"));
    assert_eq!(vertices(&mut document), before);
    let _ = std::fs::remove_file(&path);
}

/// The two verbs must not produce the same form, or one of them is mapped onto
/// the other and a sculptor has one deformer under two names.
#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "the Metal export yields no triangles; see #37"
)]
fn a_taper_and_a_twist_are_different_deformations() {
    let mut forms = Vec::new();
    for verb in DeformVerb::ALL {
        let (mut document, path) = with_mesh(&format!("{verb:?}"));
        let settings = DeformSettings {
            verb,
            axis: [0.0, 1.0, 0.0],
            span: 2.0,
            scale_start: 1.0,
            scale_end: 0.3,
            degrees: 90.0,
        };
        document
            .apply_operation(settings.operation())
            .unwrap_or_else(|e| panic!("{} was refused: {e}", verb.label()));
        forms.push(vertices(&mut document));
        let _ = std::fs::remove_file(&path);
    }
    assert_ne!(
        forms[0], forms[1],
        "a taper and a twist left the same form, so one is mapped onto the other"
    );
}

/// A field has no vertices to map forward, and the refusal has to say where the
/// deformer does apply rather than restating one representation's answer.
#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "the Metal export yields no triangles; see #37"
)]
fn a_deformer_on_a_field_is_refused_by_where_it_applies() {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let error = document
        .apply_operation(LayerOperation::Twist {
            axis: [0.0, 1.0, 0.0],
            span: 2.0,
            angle: 1.0,
        })
        .expect_err("a field has no vertices to map forward");
    assert!(
        error.to_string().contains("mesh"),
        "the refusal must name where the deformer applies: {error}"
    );
}
