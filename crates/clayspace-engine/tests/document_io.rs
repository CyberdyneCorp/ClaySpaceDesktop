//! Saving, opening, and starting over.
//!
//! The property that matters most here is not that a document round-trips —
//! it is that a *failed* open costs nothing. A sculptor who mistypes a
//! filename must still have their work.

use std::path::PathBuf;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, DocumentModel, GestureSample, OpenError, SceneModel, SculptModel, ToolKind,
};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("clayspace-document-io");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

fn fresh_document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// Where the surface sits along a direction — the fingerprint used throughout.
fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> Option<f32> {
    let n = (direction[0] * direction[0]
        + direction[1] * direction[1]
        + direction[2] * direction[2])
        .sqrt();
    let unit = direction.map(|c| c / n);
    document
        .pick(unit.map(|c| c * 4.0), unit.map(|c| -c))
        .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
}

/// A dab somewhere distinctive, so a round trip has something to preserve.
fn dab(document: &mut ClayDocument, at: [f32; 3]) {
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: at,
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("stroke");
}

#[test]
fn a_sculpt_survives_a_save_and_an_open() {
    let Some(mut document) = fresh_document() else {
        return;
    };
    let at = [0.5f32, 0.3, 0.8124];
    dab(&mut document, at);
    let sculpted = radius_along(&document, at).expect("the dab is there");

    let path = scratch("round-trip.clayspace");
    document.save(&path).expect("save");
    assert!(path.exists(), "save reported success and wrote nothing");

    let mut reopened = fresh_document().expect("a second document");
    reopened.open(&path).expect("open");

    let restored = radius_along(&reopened, at).expect("the surface came back");
    assert!(
        (sculpted - restored).abs() < 0.02,
        "the sculpt was at {sculpted} and came back at {restored}"
    );
}

#[test]
fn a_failed_open_leaves_the_work_alone() {
    // The one that matters. Everything else here is recoverable by trying
    // again; this is not.
    let Some(mut document) = fresh_document() else {
        return;
    };
    let at = [0.5f32, 0.3, 0.8124];
    dab(&mut document, at);
    let before = radius_along(&document, at).expect("the dab is there");
    let layers_before = document.scene().layers.len();

    let missing = scratch("no-such-document.clayspace");
    let error = document.open(&missing).expect_err("opening nothing succeeded");
    assert!(
        matches!(error, OpenError::NotFound(_)),
        "a missing file reported as {error:?}"
    );

    let after = radius_along(&document, at).expect("the work is still there");
    assert!(
        (before - after).abs() < 1e-4,
        "a failed open moved the surface from {before} to {after}"
    );
    assert_eq!(
        document.scene().layers.len(),
        layers_before,
        "a failed open changed the layer stack"
    );
}

#[test]
fn a_file_that_is_not_a_document_is_refused_readably() {
    let Some(mut document) = fresh_document() else {
        return;
    };
    let path = scratch("not-a-document.clayspace");
    std::fs::write(&path, b"this is not a ClaySpace document").expect("write");

    let error = document.open(&path).expect_err("garbage opened");
    assert!(
        !matches!(error, OpenError::NotFound(_)),
        "a file that exists was reported as missing"
    );
    let said = error.to_string();
    assert!(
        said.chars().any(|c| c.is_alphabetic()),
        "the refusal said nothing a user could act on: {said}"
    );
}

#[test]
fn opening_replaces_everything_that_was_there() {
    let Some(mut first) = fresh_document() else {
        return;
    };
    // Two layers in the saved document, one in the one that opens it.
    first
        .add_layer("Detalhe", clayspace_model::Representation::Sdf)
        .expect("layer");
    dab(&mut first, [0.5, 0.3, 0.8124]);
    let path = scratch("two-layers.clayspace");
    first.save(&path).expect("save");

    let mut second = fresh_document().expect("a second document");
    assert_eq!(second.scene().layers.len(), 1);
    second.open(&path).expect("open");
    assert_eq!(
        second.scene().layers.len(),
        2,
        "the opened document's layers did not replace the old ones"
    );
}

#[test]
fn a_reopened_document_can_be_sculpted_and_undone() {
    // Opening has to leave a working document, not a read-only picture of one.
    // Undo in particular: it must not reach back past the open into a document
    // the user never saw.
    let Some(mut document) = fresh_document() else {
        return;
    };
    dab(&mut document, [0.5, 0.3, 0.8124]);
    let path = scratch("then-sculpt.clayspace");
    document.save(&path).expect("save");

    let mut reopened = fresh_document().expect("a second document");
    reopened.open(&path).expect("open");
    assert!(
        !reopened.history().can_undo,
        "opening a document left something to undo"
    );

    let at = [-0.4f32, 0.2, 0.89];
    let before = radius_along(&reopened, at).expect("surface");
    dab(&mut reopened, at);
    let after = radius_along(&reopened, at).expect("surface");
    assert!(
        (before - after).abs() > 0.002,
        "a reopened document did not accept a stroke"
    );

    assert!(reopened.history().can_undo, "the stroke was not undoable");
    assert!(reopened.undo().expect("undo"), "undo did nothing");
    let undone = radius_along(&reopened, at).expect("surface");
    assert!(
        (before - undone).abs() < 0.002,
        "undo left the surface at {undone} rather than {before}"
    );
}

#[test]
fn starting_over_gives_back_the_starting_form() {
    let Some(mut document) = fresh_document() else {
        return;
    };
    let at = [0.5f32, 0.3, 0.8124];
    dab(&mut document, at);
    let sculpted = radius_along(&document, at).expect("surface");

    document.reset().expect("reset");
    let fresh = radius_along(&document, at).expect("surface");
    assert!(
        (fresh - 1.0).abs() < 0.02,
        "a reset document's surface is at {fresh}, not the unit sphere"
    );
    assert!(
        (sculpted - fresh).abs() > 0.002,
        "reset kept the sculpt: {sculpted} then {fresh}"
    );
    assert_eq!(document.scene().layers.len(), 1);

    // The engine's history legitimately holds the document's own construction
    // — adding the starting form is recorded like anything else. What must
    // hold is that a reset leaves it looking exactly like a document that was
    // just made, so the two are compared rather than one being asserted empty.
    // What the *user* can undo is the ViewModel's stack, not this one.
    let brand_new = fresh_document().expect("a fresh document");
    assert_eq!(
        document.history().depth,
        brand_new.history().depth,
        "a reset document's history is not a fresh document's"
    );
}
