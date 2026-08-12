//! Does an incremental sync still need `settle` on ClayCore 0.28.0?
//!
//! #66 changed what a subset mesh emits: it used to omit triangles straddling
//! the request boundary, and now returns every triangle with at least one
//! corner in a requested brick, attributed to the lexicographically lowest
//! requested key owning a corner.
//!
//! That attribution rule is *request-relative*, which is the part that matters
//! for a host storing per-key slots: a triangle's owner under a subset request
//! need not be its owner under a whole-surface one. So per-key equality with a
//! rebuild is the wrong question. What matters for the screen is whether the
//! *union* is the same set of triangles.

mod support;

use std::collections::HashSet;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

/// Every triangle a geometry holds, ignoring which key it was filed under.
///
/// `stored_triangles` already quantises positions and sorts each triangle's
/// corners, so a triangle is the same value however it was reached — which is
/// exactly what lets the union be compared across two different key splits.
fn triangles(geometry: &SurfaceGeometry) -> HashSet<[[i32; 3]; 3]> {
    geometry
        .stored_triangles()
        .into_values()
        .flatten()
        .collect()
}

#[test]
fn an_incremental_sync_draws_the_same_surface_a_rebuild_would() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };

    let mut incremental = SurfaceGeometry::new(&harness.gpu);
    incremental
        .rebuild(&harness.gpu, &mut document)
        .expect("first mesh");

    // Six dabs along the front, synced one at a time — a stroke, in other
    // words, without the settle at the end.
    for step in 0..6 {
        let at = [step as f32 * 0.06 - 0.15, 0.0, 1.02];
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &[GestureSample {
                    position: at,
                    pressure: 1.0,
                    time: step as f32 * 0.01,
                }],
                [false; 3],
            )
            .expect("a dab");
        incremental.sync(&harness.gpu, &mut document).expect("sync");
    }

    let mut rebuilt = SurfaceGeometry::new(&harness.gpu);
    rebuilt
        .rebuild(&harness.gpu, &mut document)
        .expect("rebuild");

    let (mine, theirs) = (triangles(&incremental), triangles(&rebuilt));
    let missing = theirs.difference(&mine).count();
    let extra = mine.difference(&theirs).count();
    println!(
        "union after six dabs: sync {} triangles, rebuild {} — {missing} missing, {extra} spare",
        mine.len(),
        theirs.len()
    );

    // Holes are what a sculptor sees, so they are the failure that matters.
    assert_eq!(
        missing, 0,
        "the incremental surface is missing {missing} triangles a rebuild has — \
         these are the seams, and `settle` is still needed"
    );
    assert_eq!(
        extra, 0,
        "the incremental surface holds {extra} triangles a rebuild does not"
    );
}
