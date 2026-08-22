//! Task 3.9, from the viewport's side: dropping to the coarse surface and
//! coming back.
//!
//! The engine half is held by `claycore_lod.rs`. This holds what the composition
//! root does with it — that a switch actually changes what is drawn, that
//! approaching restores the full surface, and the two ways the coarse path is
//! allowed to decline: no mips yet, and an edit landing while it is drawn.
//!
//! The last of those is the one worth a regression test. The two levels do not
//! share a key space — a coarse key names a 2×2×2 block of fine ones — so the
//! dirty keys the engine hands back after an edit do not address a store a
//! coarse rebuild left behind. Removing the guard was tried against this test:
//! the sync fails on an empty mesh rather than drawing anything at all.

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Detail, GestureSample, SculptModel, ToolKind};
use support::Harness;

/// A document with a starting form, or `None` where there is no engine to be
/// had — the same skip every test in this suite makes.
fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

fn dab(document: &mut ClayDocument, at: [f32; 3], time: f32) {
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: at,
                pressure: 1.0,
                time,
            }],
            [false; 3],
        )
        .expect("a dab");
}

/// A document with a settled surface and its mips built.
fn settled() -> Option<ClayDocument> {
    let mut document = document()?;
    for step in 0..6 {
        let t = step as f32 / 5.0;
        dab(&mut document, [(t - 0.5) * 0.5, 0.0, 1.0], t);
    }
    document.build_mips().expect("build the mips");
    Some(document)
}

/// Whether this document has a coarse surface at all, so a test can skip
/// rather than assert about mips that were never buildable here.
fn has_mips(document: &ClayDocument) -> bool {
    document
        .drawable_coarse_keys()
        .map(|keys| !keys.is_empty())
        .unwrap_or(false)
}

#[test]
fn dropping_to_the_coarse_surface_draws_fewer_triangles() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = settled() else {
        return;
    };
    if !has_mips(&document) {
        return;
    }

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the full surface");
    let full = geometry.triangle_count();
    assert_eq!(geometry.detail(), Detail::Full);
    assert!(full > 0, "the full surface drew nothing");

    let switched = geometry
        .set_detail(&harness.gpu, &mut document, Detail::Reduced)
        .expect("switch to the coarse surface");
    assert!(switched, "asking for a different level rebuilt nothing");
    assert_eq!(
        geometry.detail(),
        Detail::Reduced,
        "the coarse surface was available and was not taken"
    );

    let reduced = geometry.triangle_count();
    assert!(
        reduced > 0,
        "the coarse surface drew nothing, which is worse than drawing it slowly"
    );
    assert!(
        reduced < full,
        "the coarse surface was not coarser: {reduced} triangles against {full}"
    );
}

#[test]
fn approaching_restores_the_full_surface() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = settled() else {
        return;
    };
    if !has_mips(&document) {
        return;
    }

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the full surface");
    let full = geometry.triangle_count();

    geometry
        .set_detail(&harness.gpu, &mut document, Detail::Reduced)
        .expect("drop");
    assert_eq!(geometry.detail(), Detail::Reduced);

    geometry
        .set_detail(&harness.gpu, &mut document, Detail::Full)
        .expect("restore");
    assert_eq!(geometry.detail(), Detail::Full);
    assert_eq!(
        geometry.triangle_count(),
        full,
        "coming back gave a different surface than the one that was left"
    );
}

#[test]
fn asking_for_the_same_level_twice_rebuilds_nothing() {
    // The switch is a full re-mesh, so a request that changes nothing must not
    // pay for one — a resting camera asks on every frame.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = settled() else {
        return;
    };

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the full surface");

    assert!(
        !geometry
            .set_detail(&harness.gpu, &mut document, Detail::Full)
            .expect("the level already drawn"),
        "asking for the level already drawn rebuilt the surface"
    );

    geometry
        .set_detail(&harness.gpu, &mut document, Detail::Reduced)
        .expect("drop");
    assert!(
        !geometry
            .set_detail(&harness.gpu, &mut document, Detail::Reduced)
            .expect("the same level again"),
        "asking twice for the coarse surface rebuilt it a second time"
    );
}

#[test]
fn with_no_mips_the_coarse_request_draws_the_full_surface() {
    // Before any gesture has ended there is nothing coarse to draw. Drawing
    // the model slowly beats drawing an empty viewport, so the request is
    // allowed to fall back — and must say so rather than claim it was met.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };
    // A dab and no `build_mips`: the surface exists, the mips do not.
    dab(&mut document, [0.0, 0.0, 0.55], 0.0);
    assert!(
        !has_mips(&document),
        "a mip was built without anyone asking, so this proves nothing"
    );

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the full surface");
    let full = geometry.triangle_count();

    geometry
        .set_detail(&harness.gpu, &mut document, Detail::Reduced)
        .expect("ask for a coarse surface that is not there");
    assert_eq!(
        geometry.detail(),
        Detail::Full,
        "a coarse surface was reported without a mip behind it"
    );
    assert_eq!(
        geometry.triangle_count(),
        full,
        "the fallback drew something other than the full surface"
    );
}

#[test]
fn the_coarse_request_is_taken_up_once_the_mips_are_built() {
    // The other half of the fallback: it settles rather than retrying every
    // frame, so something has to ask again when the mips go up. That is what
    // the end of a gesture does.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };
    dab(&mut document, [0.0, 0.0, 0.55], 0.0);

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the full surface");
    geometry
        .set_detail(&harness.gpu, &mut document, Detail::Reduced)
        .expect("ask early");
    assert_eq!(geometry.detail(), Detail::Full, "there were no mips yet");

    document.build_mips().expect("build the mips");
    if !has_mips(&document) {
        return;
    }
    let taken = geometry
        .reapply_detail(&harness.gpu, &mut document)
        .expect("ask again");
    assert!(
        taken,
        "the coarse surface was available and still not taken"
    );
    assert_eq!(geometry.detail(), Detail::Reduced);
}

#[test]
fn an_edit_at_reduced_detail_returns_to_the_full_surface() {
    // The regression this file exists for. `sync` meshes dirty *fine* keys; a
    // store built from coarse keys cannot receive them. Verified by deleting
    // the guard: this is the only test in the file that fails, and it fails on
    // an empty mesh out of the incremental path.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = settled() else {
        return;
    };
    if !has_mips(&document) {
        return;
    }

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the full surface");
    geometry
        .set_detail(&harness.gpu, &mut document, Detail::Reduced)
        .expect("drop");
    assert_eq!(geometry.detail(), Detail::Reduced);

    dab(&mut document, [0.2, 0.0, 0.55], 1.0);
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("sync the edit");

    assert_eq!(
        geometry.detail(),
        Detail::Full,
        "an edit was meshed into the coarse store instead of returning to \
         full resolution"
    );

    // And what is drawn is what a rebuild from scratch would draw: the guard
    // has to leave the surface right, not merely leave the flag right.
    let mut rebuilt = SurfaceGeometry::new(&harness.gpu);
    rebuilt
        .rebuild(&harness.gpu, &mut document)
        .expect("a fresh full surface");
    assert_eq!(
        geometry.stored_triangles(),
        rebuilt.stored_triangles(),
        "the surface after an edit at reduced detail disagrees with a rebuild"
    );
}
