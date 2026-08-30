//! A mask survives the document it was painted on.
//!
//! `claycore_mask_persistence.rs` measured, at the boundary, that the engine
//! saves a mask attached to a layer — and recorded that this crate could not
//! use it, because the wrapper lent a mask out of a document and then asked for
//! the same document mutably, which Rust cannot spell. That was an API shape
//! rather than an engine gap, and `claycore::MaskSource` and
//! `Document::layer_mask` are the shape that fixes it.
//!
//! So this is the property that was on the table and is now a release gate:
//!
//! ```text
//! paint mask -> save -> close -> open -> the same mask, still gating
//! ```

use std::path::PathBuf;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, DocumentModel, GestureSample, MaskModel, MaskOp, Representation, SceneModel,
    SculptModel, ToolKind,
};

const FROZEN: [f32; 3] = [0.0, 0.0, 1.0];

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("clayspace-mask-persistence");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

fn opened(path: &std::path::Path) -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document.open(path).expect("reopen");
    document
}

/// Freezes a patch at the top of the form.
fn freeze(document: &mut ClayDocument, at: [f32; 3]) {
    document
        .apply_stroke(
            ToolKind::Mascara,
            BrushSettings {
                size: 0.25,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: at,
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("paint a mask");
}

/// How far the surface stands from the centre along a direction.
fn reach(document: &ClayDocument, direction: [f32; 3]) -> f32 {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    SculptModel::pick(document, unit.map(|c| c * 4.0), unit.map(|c| -c))
        .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
        .unwrap_or(f32::NAN)
}

#[test]
fn a_mask_comes_back_covering_what_it_covered() {
    let path = scratch("masked.clay");
    let mut document = document();
    freeze(&mut document, FROZEN);
    let painted = document.mask_state().painted_cells;
    assert!(painted > 0, "nothing was frozen to save");
    document.save(&path).expect("save");

    let reopened = opened(&path);
    assert_eq!(
        reopened.mask_state().painted_cells,
        painted,
        "the reopened document carries a different mask"
    );
    let value = reopened
        .mask_at(&[FROZEN])
        .expect("the reopened mask reads back")[0];
    assert!(
        value > 0.9,
        "the mask came back covering somewhere else: it reads {value} where it \
         was painted"
    );
}

#[test]
fn a_reopened_mask_still_gates_the_brushes() {
    // The half that matters. A mask that reads back and does not freeze
    // anything is a picture of a mask.
    let path = scratch("gating.clay");
    let mut document = document();
    freeze(&mut document, FROZEN);
    document.save(&path).expect("save");

    let mut reopened = opened(&path);
    let before = reach(&reopened, FROZEN);
    reopened
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.2,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: FROZEN,
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("stroke");
    let after = reach(&reopened, FROZEN);
    assert!(
        (after - before).abs() < 1e-3,
        "a stroke moved the reopened document's frozen region from {before} \
         to {after}"
    );
}

#[test]
fn a_document_with_no_mask_opens_and_carries_none() {
    let path = scratch("unmasked.clay");
    let mut document = document();
    document.save(&path).expect("save");

    let reopened = opened(&path);
    assert!(
        !reopened.mask_state().present,
        "a document nobody masked came back carrying one"
    );
    assert_eq!(reopened.mask_at(&[FROZEN]), None);
}

#[test]
fn each_subtool_keeps_its_own_mask_across_a_round_trip() {
    let path = scratch("two-subtools.clay");
    let mut document = document();
    let first = document.scene().layers[0].key;
    freeze(&mut document, FROZEN);
    let first_painted = document.mask_state().painted_cells;

    let second = document
        .add_layer("Outra", Representation::Sdf)
        .expect("a second subtool");
    assert!(
        !document.mask_state().present,
        "the new subtool inherited the first one's mask"
    );
    freeze(&mut document, [0.9, 0.0, 0.0]);
    let second_painted = document.mask_state().painted_cells;
    assert!(second_painted > 0, "the second subtool was not masked");

    document.save(&path).expect("save");
    let mut reopened = opened(&path);

    // The keys are minted afresh on open, so the subtools are found by their
    // place in the stack — which the engine answers in evaluation order.
    let keys: Vec<_> = reopened.scene().layers.iter().map(|l| l.key).collect();
    assert_eq!(keys.len(), 2, "the stack came back a different shape");
    reopened
        .set_active_layer(keys[0])
        .expect("the first subtool");
    assert_eq!(
        reopened.mask_state().painted_cells,
        first_painted,
        "the first subtool's mask came back as something else"
    );
    reopened
        .set_active_layer(keys[1])
        .expect("the second subtool");
    assert_eq!(
        reopened.mask_state().painted_cells,
        second_painted,
        "the second subtool's mask came back as something else"
    );
    let _ = (first, second);
}

#[test]
fn a_cleared_mask_stays_cleared_through_a_round_trip() {
    let path = scratch("cleared.clay");
    let mut document = document();
    freeze(&mut document, FROZEN);
    document.apply_mask_op(MaskOp::Clear).expect("clear");
    document.save(&path).expect("save");

    let reopened = opened(&path);
    assert!(
        !reopened.mask_state().is_active(),
        "a cleared mask came back freezing {} cells",
        reopened.mask_state().painted_cells
    );
}

#[test]
fn painting_a_mask_is_something_undo_can_take_back() {
    // It was not, and could not have been: a mask kept beside the document is
    // invisible to the engine's history, so an undo after a mask stroke spent
    // itself on whatever edit came before — the mask stayed and something else
    // went away. A mask attached to a layer records like every other edit.
    let mut document = document();
    freeze(&mut document, FROZEN);
    assert!(document.mask_state().is_active(), "nothing was frozen");

    assert!(document.undo().expect("undo"), "there was nothing to undo");
    assert!(
        !document.mask_state().is_active(),
        "undo left {} cells frozen",
        document.mask_state().painted_cells
    );

    assert!(document.redo().expect("redo"), "there was nothing to redo");
    assert!(
        document.mask_state().is_active(),
        "redo did not put the mask back"
    );
}
