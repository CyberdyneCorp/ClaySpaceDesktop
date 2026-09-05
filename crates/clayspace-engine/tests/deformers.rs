//! The whole-form deformers, and what one press of undo takes back.
//!
//! A deformer states something about the *form* rather than about a dab, which
//! is why it reaches the document through `apply_operation` rather than through
//! a stroke. It moves every vertex it touches in one call, so "one undo step"
//! is a real question rather than an obvious one: a mesh layer carries no
//! engine history at all, and what undo takes back is the delta record the
//! adapter keeps beside it.

use std::sync::atomic::{AtomicU32, Ordering};

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    DeformSettings, DeformVerb, ExchangeModel, ExportSettings, ImportSettings, LayerOperation,
    Representation, SceneModel, SculptModel,
};

/// A document whose active layer is a mesh, made by exporting the starting form
/// and importing it back — the only route a mesh layer has into a document.
///
/// These four tests were ignored on macOS from #35 until now. They used to
/// disable themselves — `with_mesh` returned `Option` and every step ended in
/// `?`, so a failed export returned early and the run went green — and turning
/// those into `expect` showed the Metal export coming back with no triangles at
/// all (#37), on tests that had never run there.
///
/// The ignore was over-broad by its own admission: `macOS, CPU only` could
/// always run these, and Rust cannot `cfg` on a runtime backend. It is removed
/// here rather than narrowed, because an ignore with no expiry is the quiet
/// self-disabling #35 existed to stop, wearing better manners.
fn with_mesh(who: &str) -> (ClayDocument, std::path::PathBuf) {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let path = fixture_path(who);
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

/// Where one test's fixture goes, and nowhere another's could.
///
/// The name alone is not enough, and this cost a green CI run on macOS to
/// learn. These tests run at once, each removing its own fixture on the way in
/// and on the way out; two of them asked for `taper` and `Taper`, which are the
/// same file on a **case-insensitive** filesystem — the default on macOS, and
/// not on Linux, which is why it passed here and failed there, and why it
/// failed only sometimes. One test deleted the other's mesh between its export
/// and its import, and `clay_mesh_load` reported an I/O error on a file that
/// had been there a moment earlier.
///
/// Fixed by making a collision impossible rather than by renaming the two that
/// collided: the process and a counter make every fixture its own, whatever a
/// later test decides to call itself and however the filesystem folds case.
fn fixture_path(who: &str) -> std::path::PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);

    std::env::temp_dir().join(format!(
        "clayspace-deform-{who}-{}-{}.obj",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Every vertex of the mesh layers, so a deformation can be compared exactly.
fn vertices(document: &mut ClayDocument) -> Vec<[f32; 3]> {
    document.visible_mesh_geometry().0
}

#[test]
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

/// The regression. It reproduces on every platform, which is the point: the
/// defect it guards was invisible on Linux and cost a run on macOS.
///
/// Two fixtures whose names differ only in case are one file wherever the
/// filesystem folds case, and these tests run at once and delete their own
/// fixtures. Comparing the paths as they are would pass on Linux and prove
/// nothing, so they are compared folded — the way macOS compares them.
#[test]
fn two_fixtures_never_share_a_path_however_the_filesystem_folds_case() {
    let taper = fixture_path("taper");
    let pair = fixture_path("Taper");

    assert_ne!(taper, pair);
    assert_ne!(
        taper.to_string_lossy().to_lowercase(),
        pair.to_string_lossy().to_lowercase(),
        "two fixtures differ only in case, which is one file on macOS: one \
         test would delete the other's mesh between its export and its import"
    );
}

/// And no two calls collide even when the caller asks for the same name, which
/// is what stops the next test that copies one of these from reintroducing it.
#[test]
fn the_same_name_asked_for_twice_gets_two_fixtures() {
    assert_ne!(fixture_path("taper"), fixture_path("taper"));
}
