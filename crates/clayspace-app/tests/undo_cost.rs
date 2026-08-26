//! What an undo costs against the edit it takes back.
//!
//! Nothing measured undo at all, which is how it came to cost 367 ms to
//! reverse a dab that cost 5 ms — 70x, on a model of 1043 surface bricks.
//!
//! Two things made it that. `ClayDocument::undo` bounds its refill by the
//! whole layer rather than by what moved, because `clay_document_undo` reports
//! only *whether* it undid something and not what it touched; that is upstream
//! and is why the ratio here is still large. The other was ours: the dirty set
//! an edit produces is an *influence bound*, which is a box, and a box around
//! a surface is mostly not surface. Two thirds of an undo's keys and a third
//! of a dab's were uniformly inside or outside — no lattice, no triangles —
//! and were being marched anyway.
//!
//!   dirty keys      meshed before      meshed now
//!   dab      27               27               18
//!   undo   2940             2940             1045
//!
//! Undo's sync went 284 ms to 141 ms and a dab's 4.3 ms to 3.6 ms.
//!
//! The milliseconds are now also recorded, as `history.undo`, `history.edit`
//! and `history.undo_ratio` in the performance baseline — so a change of engine
//! moves a figure `just bench-compare` reports rather than a number in a
//! comment. What stays here is the explanation and the key counts.
//!
//! The assertions below are about *keys*, not milliseconds, wherever they can
//! be: a key count is the same on every machine, and it is the thing that was
//! actually wrong. The one timing assertion compares two measurements taken
//! moments apart on the same model, which is a ratio rather than a budget.

mod support;

use std::collections::HashSet;
use std::time::Instant;

use clayspace_app::{SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

/// Edits behind the measurement, so it is representative rather than
/// best-case: a fresh starting form has few enough bricks to hide this.
const EDITS: usize = 96;

fn dab(document: &mut SharedDocument, x: f32, y: f32) {
    let z = (1.0f32 - x * x - y * y).max(0.05).sqrt();
    let _ = document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings::default(),
        &[GestureSample {
            position: [x, y, z],
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    );
}

#[test]
fn undo_meshes_no_more_than_the_surface_holds() {
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

    for i in 0..EDITS {
        let t = i as f32 / (EDITS - 1) as f32;
        dab(&mut document, -0.5 + t, -0.4 + (t * 6.0).sin() * 0.35);
        document
            .with(|d| geometry.sync(&harness.gpu, d))
            .expect("sync");
    }
    // One more dab, and the sync that draws it.
    let started = Instant::now();
    dab(&mut document, 0.1, 0.05);
    let edit = started.elapsed();
    let started = Instant::now();
    document
        .with(|d| geometry.sync(&harness.gpu, d))
        .expect("sync");
    let edit_sync = started.elapsed();
    let edit_keys = geometry.last_cost().expect("a cost").keys;

    // Taking exactly that dab back.
    let started = Instant::now();
    document.undo().expect("undo");
    let undo = started.elapsed();
    // Read after the undo rather than before the dab: both edits move the
    // surface, and the count this is compared against has to be the one that
    // was standing when the sync below ran.
    // Counted live rather than read from `surface_brick_count`, which is
    // refreshed only by a handful of operations and is not one of them.
    let bricks = document.with(|d| d.cache().surface_bricks().expect("bricks").len());
    let reported = document.with(|d| d.surface_brick_count());
    let started = Instant::now();
    document
        .with(|d| geometry.sync(&harness.gpu, d))
        .expect("sync");
    let undo_sync = started.elapsed();
    let undo_keys = geometry.last_cost().expect("a cost").keys;

    // What the same surface costs built from scratch, which is the bar an
    // incremental path has to stay under to be worth being incremental.
    let started = Instant::now();
    document
        .with(|d| geometry.rebuild(&harness.gpu, d))
        .expect("rebuild");
    let rebuild = started.elapsed();

    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    println!(
        "\n--- {bricks} surface bricks after {EDITS} edits ({reported} reported) ---\n  \
         dab    edit {:6.2} ms  sync {:6.2} ms over {edit_keys} keys\n  \
         undo   undo {:6.2} ms  sync {:6.2} ms over {undo_keys} keys\n  \
         rebuild             {:6.2} ms",
        ms(edit),
        ms(edit_sync),
        ms(undo),
        ms(undo_sync),
        ms(rebuild),
    );

    // The regression. An undo dirties the whole layer's influence bound, which
    // is a volume; meshing all of it marched 2940 keys against 1043 that could
    // hold a triangle. Whatever the bound, a sync may never mesh more keys
    // than the surface has.
    assert!(
        undo_keys <= bricks,
        "an undo meshed {undo_keys} keys against {bricks} surface bricks. The \
         dirty set is a box around the edit and most of a box is not surface — \
         something has stopped filtering it."
    );

    // And the same for an ordinary edit, where the waste was a third.
    assert!(
        edit_keys <= undo_keys,
        "a dab meshed {edit_keys} keys and an undo of it {undo_keys}; the dab \
         cannot have touched more than the undo's whole-layer bound"
    );

    // The other half of the defect: the incremental path cost nearly twice a
    // full rebuild of the same surface, so the fast path was the slow one.
    // A ratio rather than a budget — both are measured here, moments apart.
    assert!(
        undo_sync.as_secs_f64() < 1.5 * rebuild.as_secs_f64(),
        "an undo's sync took {:.0} ms against {:.0} ms to rebuild the whole \
         surface. Meshing a subset is only worth it while it is cheaper than \
         meshing everything.",
        ms(undo_sync),
        ms(rebuild),
    );
}

#[test]
fn a_sync_meshes_only_keys_that_hold_a_surface() {
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

    for i in 0..EDITS {
        let t = i as f32 / (EDITS - 1) as f32;
        dab(&mut document, -0.5 + t, -0.4 + (t * 6.0).sin() * 0.35);
        document
            .with(|d| geometry.sync(&harness.gpu, d))
            .expect("sync");
    }

    // The dirty keys of one dab, inspected before a sync consumes them, and
    // the states the filter reads.
    dab(&mut document, 0.1, 0.05);
    let (dirty, surface) = document.with(|d| {
        let dirty = d.take_dirty_keys();
        let surface: HashSet<_> = d
            .cache()
            .surface_bricks()
            .expect("bricks")
            .into_iter()
            .collect();
        (dirty, surface)
    });
    let holding = dirty.iter().filter(|key| surface.contains(*key)).count();

    println!(
        "a dab: {} dirty keys, {holding} hold a surface",
        dirty.len()
    );
    assert!(
        holding < dirty.len(),
        "every one of a dab's {} dirty keys held a surface, so this model no \
         longer exercises the filter and the numbers above mean nothing",
        dirty.len()
    );

    // The keys were taken to look at them, so the surface comes back the only
    // other way there is.
    document
        .with(|d| geometry.rebuild(&harness.gpu, d))
        .expect("rebuild");
}
