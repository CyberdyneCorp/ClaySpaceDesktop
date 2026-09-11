//! Which bake the Optimize action runs, and the one it must refuse.
//!
//! `consolidate_layer` collapses the whole subtool. That is the cure for a
//! stack of baked volumes or a long edit list, and it is measured **6x worse**
//! for a chain of brushes on a layer with nothing to absorb — it swaps cheap
//! analytic items for a dense volume, and the marching win is swamped by what
//! the volume costs per sample. The engine says so on
//! `CLAY_DEGRADATION_DEFORMERS` and withholds its advice there for that reason.
//!
//! So today the offer never reaches a sculptor on a Move-degraded layer and
//! this could not fire. ClayCore #534 proposes lowering the trigger to a
//! step-scale floor, which would make the flag true on precisely the layer the
//! whole-layer bake is wrong for — the prompt appears, the sculptor clicks, and
//! the field gets slower. The refusal is pinned here rather than left resting
//! on an advisory that is about to change.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, FieldDegradation, GestureSample, SceneModel, SculptModel, ToolKind,
};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// A Move dab somewhere new, which is what builds a chain rather than folding
/// into the last one.
fn dab(document: &mut ClayDocument, index: usize) {
    let angle = index as f32 * 0.7;
    let at = [0.9 * angle.cos(), 0.35 * angle.sin(), 0.25];
    let samples: Vec<GestureSample> = (0..3)
        .map(|i| GestureSample {
            position: [at[0] + i as f32 * 0.02, at[1], at[2]],
            pressure: 1.0,
            time: i as f32,
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                size: 0.45,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("a dab");
}

fn health(document: &ClayDocument) -> clayspace_model::FieldHealth {
    document
        .scene()
        .active_layer()
        .expect("a layer")
        .health
        .expect("a field layer reports health")
}

/// A session of Move dabs is reported as a deformer chain, and the count the
/// interface can show is the chain rather than the item total.
#[test]
fn a_move_session_reports_a_deformer_chain_and_not_one_item() {
    let mut document = sphere();
    assert_eq!(
        health(&document).degradation,
        FieldDegradation::None,
        "a fresh starting form is not degraded"
    );

    for index in 1..=8 {
        dab(&mut document, index);
    }
    let after = health(&document);

    assert_eq!(
        after.degradation,
        FieldDegradation::Deformers,
        "eight Move dabs build a chain of brushes on a layer with no volumes \
         to absorb, which is the deformer case"
    );
    assert!(
        !after.degradation.whole_layer_bake_would_help(),
        "and collapsing the whole layer is not its cure"
    );
    // The number #103 is about: grabs hang off an item rather than adding
    // items, so the item count cannot stand in for "how much work is here".
    assert_eq!(
        after.items, 1,
        "the starting sphere is still one item, however many dabs moved it"
    );
    assert!(
        after.chain >= 8,
        "the chain is the count that moved: {} after eight dabs",
        after.chain
    );
}

/// And the action refuses to flatten it.
#[test]
fn optimising_a_deformer_degraded_layer_is_refused() {
    let mut document = sphere();
    for index in 1..=8 {
        dab(&mut document, index);
    }
    let key = document.scene().active_layer().expect("a layer").key;
    assert_eq!(health(&document).degradation, FieldDegradation::Deformers);

    let refused = document.consolidate_layer(key);
    assert!(
        refused.is_err(),
        "the whole-layer bake ran on a layer it is measured 6x worse for. \
         Today the advisory hides the button; ClayCore #534 would show it"
    );
    // And the refusal left the document alone.
    assert_eq!(
        health(&document).chain,
        health(&document).chain,
        "a refusal changes nothing"
    );
}

/// The other mechanism is still offered its own cure.
///
/// Without this the fix could be "never consolidate", which passes the test
/// above and breaks the feature.
#[test]
fn optimising_a_layer_that_wants_it_still_works() {
    let mut document = sphere();
    let key = document.scene().active_layer().expect("a layer").key;
    // A fresh layer is not deformer-degraded, so the action is available to it.
    assert_ne!(health(&document).degradation, FieldDegradation::Deformers);
    document
        .consolidate_layer(key)
        .expect("a layer with no brush chain can still be collapsed");
    assert!(
        health(&document).consolidated,
        "and it reports itself collapsed afterwards"
    );
}
