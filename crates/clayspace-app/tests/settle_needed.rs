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

#[test]
fn settlement_tracks_partial_requests_and_complete_replacements() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let policy = BackendPolicy::discover(None).expect("CPU policy");
    let mut empty = ClayDocument::new(policy.clone()).expect("empty document");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("sphere");
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("initial sync");
    assert!(
        !geometry.needs_settle(),
        "one initial request is already consistent"
    );
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: [0.0, 0.0, 1.02],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("dab");
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("partial sync");
    assert!(
        geometry.needs_settle(),
        "partial ownership must still be settled"
    );
    geometry.settle_layout(&harness.gpu);
    assert!(
        geometry.needs_settle(),
        "exact duplicate pruning is not a full remesh"
    );
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("full rebuild");
    assert!(!geometry.needs_settle());
    geometry
        .rebuild(&harness.gpu, &mut empty)
        .expect("empty rebuild");
    assert_eq!(geometry.triangle_count(), 0);
    assert!(!geometry.needs_settle());
}

#[test]
fn committing_a_live_preview_already_replaces_the_surface() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let policy = BackendPolicy::discover(None).expect("CPU policy");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("sphere");
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("initial sync");
    assert!(document.open_live_gesture(ToolKind::Mover, [false; 3]));
    document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings::default(),
            &[
                GestureSample {
                    position: [0.0, 0.0, 1.0],
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [0.1, 0.0, 1.0],
                    pressure: 1.0,
                    time: 0.01,
                },
            ],
            [false; 3],
        )
        .expect("preview");
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("preview sync");
    document.close_live_gesture().expect("commit");
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("committed epoch sync");
    assert!(
        !geometry.needs_settle(),
        "the epoch change already rebuilt every key"
    );
    let mut expected = SurfaceGeometry::new(&harness.gpu);
    expected
        .rebuild(&harness.gpu, &mut document)
        .expect("reference rebuild");
    let mut actual = geometry.stored_triangles_exact();
    let mut reference = expected.stored_triangles_exact();
    actual.sort_unstable();
    reference.sort_unstable();
    assert_eq!(
        actual, reference,
        "skipping another settle must preserve every attribute"
    );
}

#[test]
fn ordinary_releases_compact_exactly_without_meshing_again() {
    let Some(harness) = Harness::new() else {
        return;
    };
    for tool in [
        ToolKind::Padrao,
        ToolKind::Inflar,
        ToolKind::Argila,
        ToolKind::Vinco,
        ToolKind::Camada,
        ToolKind::Planar,
        ToolKind::Puxar,
    ] {
        let policy = BackendPolicy::discover(None).expect("policy");
        let mut document = ClayDocument::new(policy)
            .and_then(ClayDocument::with_starting_form)
            .expect("sphere");
        let mut geometry = SurfaceGeometry::new(&harness.gpu);
        geometry
            .rebuild(&harness.gpu, &mut document)
            .expect("initial");
        for release in 0..2 {
            for step in 0..6 {
                let t = (release * 6 + step) as f32 / 11.0;
                document
                    .apply_stroke(
                        tool,
                        BrushSettings::default(),
                        &[
                            GestureSample {
                                position: [(t - 0.5) * 0.7, (t * 12.0).sin() * 0.12, 1.0],
                                pressure: 1.0,
                                time: t,
                            },
                            GestureSample {
                                position: [
                                    (t - 0.5) * 0.7 + 0.015,
                                    (t * 12.0).sin() * 0.12 + 0.01,
                                    1.01,
                                ],
                                pressure: 1.0,
                                time: t + 0.01,
                            },
                        ],
                        [false; 3],
                    )
                    .expect("dab");
                geometry.sync(&harness.gpu, &mut document).expect("sync");
            }
            geometry
                .settle_after_edit(&harness.gpu, &mut document)
                .expect("release");
            let cost = geometry.last_settle().expect("settlement telemetry");
            assert_eq!(cost.route, clayspace_app::SettleRoute::Compact, "{tool:?}");
            assert_eq!(cost.engine_mesh_time, std::time::Duration::ZERO);
            assert_eq!(cost.read_time, std::time::Duration::ZERO);
            assert!(!geometry.needs_settle());
            let mut reference = SurfaceGeometry::new(&harness.gpu);
            reference
                .rebuild(&harness.gpu, &mut document)
                .expect("reference");
            let mut actual = geometry.stored_triangles_exact();
            actual.sort_unstable();
            let mut expected = reference.stored_triangles_exact();
            expected.sort_unstable();
            assert!(
                actual == expected,
                "{tool:?}, release {release}: all attributes and multiplicities"
            );
        }
    }
}

#[test]
fn release_compaction_defers_preview_and_preserves_full_rebuild_routes() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let policy = BackendPolicy::discover(None).expect("policy");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("sphere");
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.sync(&harness.gpu, &mut document).expect("initial");
    assert!(document.open_live_gesture(ToolKind::Mover, [false; 3]));
    document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings::default(),
            &[
                GestureSample {
                    position: [0.0, 0.0, 1.0],
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [0.1, 0.0, 1.0],
                    pressure: 1.0,
                    time: 0.01,
                },
            ],
            [false; 3],
        )
        .expect("preview");
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("preview sync");
    let before: HashSet<_> = geometry.stored_triangles_exact().into_iter().collect();
    geometry
        .settle_after_edit(&harness.gpu, &mut document)
        .expect("defer");
    assert!(
        geometry.last_settle().is_none(),
        "an open preview must not be settled"
    );
    let after: HashSet<_> = geometry.stored_triangles_exact().into_iter().collect();
    assert!(before == after, "the preview must remain unchanged");
    document.close_live_gesture().expect("commit");
    // Deliberately omit sync: the retained preview has a different epoch and
    // face shading, so compaction cannot turn it into document geometry.
    geometry
        .settle_after_edit(&harness.gpu, &mut document)
        .expect("fallback");
    assert_eq!(
        geometry.last_settle().unwrap().route,
        clayspace_app::SettleRoute::Bricks
    );
    let mut reference = SurfaceGeometry::new(&harness.gpu);
    reference
        .rebuild(&harness.gpu, &mut document)
        .expect("reference");
    let mut actual = geometry.stored_triangles_exact();
    actual.sort_unstable();
    let mut expected = reference.stored_triangles_exact();
    expected.sort_unstable();
    assert!(
        actual == expected,
        "fallback must install the committed field"
    );
    geometry
        .settle_after_edit(&harness.gpu, &mut document)
        .expect("eligible");
    assert_eq!(
        geometry.last_settle().unwrap().route,
        clayspace_app::SettleRoute::Compact
    );
    geometry
        .settle(&harness.gpu, &mut document)
        .expect("explicit settle");
    assert_eq!(
        geometry.last_settle().unwrap().route,
        clayspace_app::SettleRoute::Bricks
    );
}
