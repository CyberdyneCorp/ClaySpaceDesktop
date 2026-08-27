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

fn fresh_document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Where the surface sits along a direction — the fingerprint used throughout.
fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> Option<f32> {
    let n =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
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
    let mut document = fresh_document();
    let at = [0.5f32, 0.3, 0.8124];
    dab(&mut document, at);
    let sculpted = radius_along(&document, at).expect("the dab is there");

    let path = scratch("round-trip.clayspace");
    document.save(&path).expect("save");
    assert!(path.exists(), "save reported success and wrote nothing");

    let mut reopened = fresh_document();
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
    let mut document = fresh_document();
    let at = [0.5f32, 0.3, 0.8124];
    dab(&mut document, at);
    let before = radius_along(&document, at).expect("the dab is there");
    let layers_before = document.scene().layers.len();

    let missing = scratch("no-such-document.clayspace");
    let error = document
        .open(&missing)
        .expect_err("opening nothing succeeded");
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
    let mut document = fresh_document();
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
    let mut first = fresh_document();
    // Two layers in the saved document, one in the one that opens it.
    first
        .add_layer("Detalhe", clayspace_model::Representation::Sdf)
        .expect("layer");
    dab(&mut first, [0.5, 0.3, 0.8124]);
    let path = scratch("two-layers.clayspace");
    first.save(&path).expect("save");

    let mut second = fresh_document();
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
    let mut document = fresh_document();
    dab(&mut document, [0.5, 0.3, 0.8124]);
    let path = scratch("then-sculpt.clayspace");
    document.save(&path).expect("save");

    let mut reopened = fresh_document();
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
    let mut document = fresh_document();
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
    let brand_new = fresh_document();
    assert_eq!(
        document.history().depth,
        brand_new.history().depth,
        "a reset document's history is not a fresh document's"
    );
}

#[test]
fn a_reopened_document_keeps_its_layers_as_they_were() {
    // ClayCore #69, from this side. Until 0.29.0 there was no enumeration at
    // all: this host probed consecutive ids for one that answered
    // `clay_layer_bounds`, regenerated names, treated every layer as SDF, and
    // — the half that is a correctness bug rather than a cosmetic one — lost
    // stack order, so a reopened document could evaluate differently from the
    // one saved.
    let mut document = fresh_document();

    let detail = document
        .add_layer("Detalhe_fino", clayspace_model::Representation::Sdf)
        .expect("a second layer");
    document
        .add_voxel_layer("Voxels", 0.05)
        .expect("a voxel layer");
    document
        .set_layer_visible(detail, false)
        .expect("hide the detail layer");

    // Deliberately not the order they were added in: order is the thing most
    // likely to be lost, so the test has to make it distinguishable.
    let voxels = document
        .scene()
        .layers
        .iter()
        .find(|l| l.name == "Voxels")
        .map(|l| l.key)
        .expect("the voxel layer is in the scene");
    document.move_layer(voxels, 0).expect("move to the bottom");

    let before: Vec<(String, clayspace_model::Representation, bool)> = document
        .scene()
        .layers
        .iter()
        .map(|l| (l.name.clone(), l.representation, l.visible))
        .collect();

    let path = scratch("layers-roundtrip.clayspace");
    document.save(&path).expect("save");
    document.open(&path).expect("open");

    let after: Vec<(String, clayspace_model::Representation, bool)> = document
        .scene()
        .layers
        .iter()
        .map(|l| (l.name.clone(), l.representation, l.visible))
        .collect();

    assert_eq!(
        after, before,
        "a reopened document does not hold the layers it was saved with"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn voxel_work_survives_a_save() {
    // It did not, and #69 is what made it visible. `add_voxel_layer` used to
    // create an *SDF* layer in the document and keep a standalone grid beside
    // it, so the engine reported SDF perfectly correctly — because that is
    // what the layer was — and nothing voxel was ever written to the file.
    // Silent data loss: the sculptor saw their work, saved, reopened, and it
    // was gone.
    let mut document = fresh_document();
    document
        .add_voxel_layer("Voxels", 0.05)
        .expect("a voxel layer");

    let brush = BrushSettings {
        size: 0.25,
        ..Default::default()
    };
    let mut deposited = false;
    for step in 0..6 {
        let t = step as f32 / 5.0;
        let outcome = document
            .apply_stroke(
                ToolKind::Padrao,
                brush,
                &[GestureSample {
                    position: [(t - 0.5) * 0.4, 0.0, 0.0],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("deposit");
        deposited |= outcome.changed;
    }
    assert!(
        deposited,
        "nothing was deposited, so there is nothing to lose"
    );

    let path = scratch("voxel-roundtrip.clayspace");
    document.save(&path).expect("save");
    document.open(&path).expect("open");

    let layer = document
        .scene()
        .layers
        .iter()
        .find(|l| l.name == "Voxels")
        .cloned()
        .expect("the voxel layer came back");
    assert_eq!(
        layer.representation,
        clayspace_model::Representation::Voxel,
        "the layer came back as something other than a voxel layer, which \
         means its grid is not in the document"
    );

    // And it is still sculptable: a voxel verb on it must be accepted rather
    // than refused for the representation.
    document
        .set_active_layer(layer.key)
        .expect("select the voxel layer");
    document
        .apply_stroke(
            ToolKind::Raspar,
            brush,
            &[GestureSample {
                position: [0.0, 0.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("a voxel verb on a reopened voxel layer");
    let _ = std::fs::remove_file(&path);
}
