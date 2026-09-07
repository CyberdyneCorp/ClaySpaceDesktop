//! Does the cut's refill name every brick it changed?
//!
//! `clayspace-engine/tests/cut.rs` beside this one reads the *field*, which is
//! answered from the tape and stays correct whether or not the brick cache was
//! refilled. What a sculptor sees is the meshed surface, and that is built
//! from the cache — so a cut whose refill named too small a region shows the
//! removed material still standing until something else happens to dirty it.
//! That is the failure this asks about, and the field cannot see it.
//!
//! The control is a second document that is cut *before* its first mesh, so
//! its cache is filled from scratch with the cut already in the tape. Nothing
//! narrow runs on that path, which is what makes it ground truth. Two renders
//! of the same form should be the same picture.
//!
//! Proven by breaking it: with the cut's `refill_bound` removed, 2594 pixels
//! differ where the passing tree differs by none. The triangle counts in that
//! run were **identical** — 281,362 either way — which is the reason this
//! compares a picture. A brick holding the material a cut removed holds about
//! as many triangles as one that does not; it holds them somewhere else.

mod support;

use clayspace_app::{SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{CutGesture, CutModel, DrawnCut, OutlineFrame, SculptModel};
use clayspace_view::Camera;
use support::Harness;

fn document() -> Option<SharedDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    Some(SharedDocument::new(document))
}

/// A frame looking along -Z, as a camera at the front gives.
fn frame() -> OutlineFrame {
    OutlineFrame {
        origin: [0.0; 3],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        scale: [1.0, 1.0],
    }
}

/// A line across the form, low enough that what it takes is a wide slice
/// rather than a cap — a cut that removes little could be missed by a
/// threshold that a stale cache would otherwise fail.
fn cut() -> DrawnCut {
    DrawnCut {
        track: vec![[-2.0, -0.25], [2.0, -0.25]],
        frame: frame(),
        gesture: CutGesture::Line,
    }
}

#[test]
fn a_cut_leaves_no_bricks_holding_the_material_it_removed() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let (Some(live), Some(fresh)) = (document(), document()) else {
        return;
    };

    // Framed on the form before either is cut, so both renders stand in the
    // same place. A camera framed afterwards would move with whichever bounds
    // it was given and the two pictures would differ for that reason alone.
    let mut camera = Camera::default();
    match SculptModel::bounds(&live) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }

    // The path a sculptor takes: the form is already on screen when the cut
    // lands, so only what the cut marks is refilled.
    let mut drawn = SurfaceGeometry::new(&harness.gpu);
    if live.with(|d| drawn.rebuild(&harness.gpu, d)).is_err() {
        return;
    }
    live.with(|d| d.apply_cut(&cut()))
        .expect("the cut lands on the starting form");
    let _ = live.with(|d| drawn.sync(&harness.gpu, d));
    let incremental = harness.capture(drawn.mesh(), &camera, false, "cut-incremental");

    // The control: cut first, then fill the whole cache from the tape.
    let mut whole = SurfaceGeometry::new(&harness.gpu);
    fresh
        .with(|d| d.apply_cut(&cut()))
        .expect("the same cut on the same form");
    if fresh.with(|d| whole.rebuild(&harness.gpu, d)).is_err() {
        return;
    }
    let from_scratch = harness.capture(whole.mesh(), &camera, false, "cut-from-scratch");

    let differing = incremental
        .pixels
        .chunks_exact(4)
        .zip(from_scratch.pixels.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 8))
        .count();
    println!(
        "{} triangles cut incrementally, {} from scratch; {differing} pixels differ",
        drawn.triangle_count(),
        whole.triangle_count()
    );
    assert!(
        differing < 40,
        "{differing} pixels of the drawn surface differ from the same cut made \
         before the first mesh, so placing the cut left bricks holding the \
         material it removed. See target/visual/cut-incremental.png"
    );
}
