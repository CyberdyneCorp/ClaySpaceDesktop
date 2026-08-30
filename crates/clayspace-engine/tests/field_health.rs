//! The engine's advice about a worked field layer, and what it costs to ask.
//!
//! A chain of edits steepens the field it produces: each bake resamples what
//! the last one left until a ray march takes many small steps and every dab
//! pays for it. The engine measures that and says when collapsing the layer is
//! worth it — and until now nothing read the answer. `layer_cost`, the one
//! call that carried it, had no caller outside a benchmark.
//!
//! Part of the reason is in the second test: that call also estimates what
//! collapsing would occupy, and the estimate is four orders of magnitude more
//! expensive than the advice. Anything asking on every refresh had to be the
//! cheap half alone.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SceneModel, SculptModel, ToolKind};

/// How many strokes the fixtures make.
///
/// The engine starts advising at eight — measured, a nine-item layer already
/// reports a safe step scale of 0.016 against the 0.5 asked about — so this is
/// a little past the threshold rather than the ninety-six a session reaches.
/// Collapsing a layer costs seconds and grows with the edit list, and a test
/// that measured the phenomenon at session scale spent two and a half minutes
/// establishing what twelve strokes establish.
const WORKED: usize = 12;

fn worked_form(edits: usize) -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    for step in 0..edits {
        let t = step as f32 / edits.max(2) as f32;
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.2,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: [-0.5 + t, 0.0, 1.0],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("a dab");
    }
    document
}

fn health(document: &mut ClayDocument) -> clayspace_model::FieldHealth {
    document.scene().layers[0]
        .health
        .expect("a field layer reports its health")
}

#[test]
fn a_worked_layer_says_it_has_become_costly() {
    let mut fresh = worked_form(0);
    assert!(
        !health(&mut fresh).advises_consolidation,
        "a fresh form should not be advising anything"
    );

    let mut worked = worked_form(WORKED);
    let after = health(&mut worked);
    assert!(
        after.advises_consolidation,
        "96 strokes left the layer at {} items and a safe step scale of {}, \
         and the engine is not advising: the advice is not reaching the scene",
        after.items, after.safe_step_scale,
    );
    assert!(
        after.items > WORKED as i32,
        "the edit list should hold roughly one item per stroke, not {}",
        after.items
    );
}

#[test]
fn collapsing_the_layer_takes_the_advice_away() {
    let mut document = worked_form(WORKED);
    let key = document.scene().layers[0].key;
    assert!(health(&mut document).advises_consolidation);

    document.consolidate_layer(key).expect("collapse it");

    let after = health(&mut document);
    assert!(
        after.consolidated,
        "the layer was collapsed and does not say so, so the row offering it \
         would go on offering it"
    );
    assert!(
        !after.advises_consolidation,
        "still advising after collapsing: {} items, safe step scale {}",
        after.items, after.safe_step_scale
    );
}

#[test]
fn asking_the_scene_for_health_is_not_asking_what_collapsing_costs() {
    let document = worked_form(WORKED);
    let key = document.scene().layers[0].key;
    let started = std::time::Instant::now();
    let _ = document.scene();
    let scene = started.elapsed();

    // The same question plus the byte estimate, which is what `layer_cost`
    // asks and what makes it unusable on a refresh path. Measured on this
    // fixture: 33 µs against 287 ms.
    let started = std::time::Instant::now();
    let _ = document.layer_cost(key).expect("the full cost");
    let full = started.elapsed();

    assert!(
        scene < full / 10,
        "assembling the scene took {scene:?} against {full:?} for the full \
         cost. The scene is built on every refresh and the estimate is not \
         something it can afford — if these are the same call again, the \
         interface has just become a slideshow"
    );
}
