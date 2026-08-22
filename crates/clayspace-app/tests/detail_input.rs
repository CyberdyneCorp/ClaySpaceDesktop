//! The brick count the level-of-detail policy decides on tracks the sculpt.
//!
//! It did not. `refresh_stats` was called by opening a document, building the
//! starting form, attaching a reference, baking a mask and placing an armature
//! — and by nothing a sculptor does continuously. So `surface_brick_count`
//! stayed at whatever the starting form produced for the rest of the session,
//! while the surface grew underneath it.
//!
//! That number is the policy's second input, including the floor below which a
//! model is never coarsened. How far it drifted depended on how far the sculpt
//! travelled from its starting form — which is to say the defect was largest
//! exactly where the feature matters. Nothing looked wrong either way: a level
//! that never switches is indistinguishable from a level that need not.

mod support;

use clayspace_app::{SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

fn dab(document: &mut SharedDocument, x: f32, y: f32) {
    let z = (1.0f32 - x * x - y * y).max(0.05).sqrt();
    let _ = document.apply_stroke(
        ToolKind::Padrao,
        // A brush big enough to move the count rather than dimple the form.
        // Which way it moves is not the point and is not asserted: broad
        // strokes merge bricks as often as they add them.
        BrushSettings {
            size: 0.35,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: [x, y, z],
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    );
}

#[test]
fn sculpting_moves_the_count_the_policy_reads() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(doc) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form) else {
        return;
    };
    let mut document = SharedDocument::new(doc);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    document
        .with(|d| geometry.rebuild(&harness.gpu, d))
        .expect("first mesh");

    let counted = |document: &SharedDocument| {
        document.with(|d| {
            (
                d.surface_brick_count(),
                d.cache().surface_bricks().expect("bricks").len(),
            )
        })
    };

    let (reported, live) = counted(&document);
    assert_eq!(
        reported, live,
        "the count is wrong before anything was drawn"
    );

    // Enough sculpting to move the surface well past where it started.
    for i in 0..96 {
        let t = i as f32 / 95.0;
        dab(&mut document, -0.5 + t, -0.4 + (t * 6.0).sin() * 0.35);
        document
            .with(|d| geometry.sync(&harness.gpu, d))
            .expect("sync");
    }

    let (reported, live) = counted(&document);
    println!("after 96 edits: {reported} reported, {live} live");
    assert_eq!(
        reported, live,
        "the level-of-detail policy is being handed {reported} surface bricks \
         while the cache holds {live}. The statistics are no longer refreshed \
         where edits happen."
    );

    // And an undo, which moves the surface without adding a node.
    document.undo().expect("undo");
    document
        .with(|d| geometry.sync(&harness.gpu, d))
        .expect("sync");
    let (reported, live) = counted(&document);
    println!("after an undo: {reported} reported, {live} live");
    assert_eq!(reported, live, "an undo left the count behind");
}
