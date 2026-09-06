//! Does a narrowed refill leave the *cache* stale?
//!
//! The engine tests beside this one compare picks, and a pick is answered from
//! a path that stays correct whether or not the brick cache was refilled — so
//! they detect an edit that never happened and are blind to a region that was
//! too small. Checked, not assumed: with the drag marking **no region at all**
//! they still pass, and only fail when the re-sweep itself is removed.
//!
//! What a sculptor sees is the *meshed* surface, and that is built from the
//! cache. So this compares triangles: drag, mesh, then dirty every brick the
//! layer reaches and mesh again. A region that named everything it should
//! leaves the second mesh identical to the first.

mod support;

use clayspace_app::{SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{CurveJoin, CurveModel, SculptModel};
use clayspace_view::Camera;
use support::Harness;

/// The drawn surface, rendered.
///
/// A picture rather than a triangle count: a stale brick usually holds the
/// *same number* of triangles in the wrong place, so a count is exactly the
/// measurement that cannot see this. What is rendered comes from the mesh,
/// which comes from the cache, which is the thing under test.

#[test]
fn a_dragged_control_point_leaves_no_stale_bricks() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form) else {
        return;
    };
    let document = SharedDocument::new(document);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    if document
        .with(|d| geometry.rebuild(&harness.gpu, d))
        .is_err()
    {
        return;
    }

    let mut camera = Camera::default();
    match SculptModel::bounds(&document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }

    document.with(|d| d.begin_curve());
    for step in 0..16 {
        let t = step as f32 / 16.0;
        document
            .with(|d| {
                d.add_curve_point(
                    [
                        -1.3 + t * 2.6,
                        1.35 + (t * 7.0).sin() * 0.45,
                        (t * 5.0).cos() * 0.25,
                    ],
                    0.09,
                )
            })
            .expect("refused");
    }
    let _ = document.with(|d| geometry.sync(&harness.gpu, d));

    // Drag one point well clear of where it started, so the place it left and
    // the place it arrived do not overlap.
    document.with(|d| d.select_curve_point(Some(8)));
    for _ in 0..6 {
        document
            .with(|d| d.drag_curve([0.0, 0.09, 0.0]))
            .expect("drag");
    }
    let _ = document.with(|d| geometry.sync(&harness.gpu, d));
    let dragged = harness.capture(geometry.mesh(), &camera, false, "curve-drag-incremental");
    let dragged_triangles = geometry.triangle_count();

    // Every brick the tube reaches, refilled: a join away and back is neither
    // an append nor a drag, so it takes the node's own bound.
    let join = document.with(|d| d.curve().join);
    let other = if join == CurveJoin::Corners {
        CurveJoin::Through
    } else {
        CurveJoin::Corners
    };
    document.with(|d| d.set_curve_join(other)).expect("away");
    document.with(|d| d.set_curve_join(join)).expect("back");
    let _ = document.with(|d| geometry.sync(&harness.gpu, d));
    let whole = harness.capture(geometry.mesh(), &camera, false, "curve-drag-refilled");

    let differing = dragged
        .pixels
        .chunks_exact(4)
        .zip(whole.pixels.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 8))
        .count();
    println!(
        "{dragged_triangles} triangles after the drag, {} after a full refill; \
         {differing} pixels differ",
        geometry.triangle_count()
    );
    assert!(
        differing < 40,
        "{differing} pixels of the drawn surface change when every brick is \
         refilled, so dragging a control point left bricks holding an older \
         shape — most likely where the point came from rather than where it \
         went. See target/visual/curve-drag-incremental.png"
    );
}

/// The same question for the *append* path, which is a different region with a
/// margin of its own.
///
/// Its engine-side guard compares picks, and a pick is answered from a path
/// that stays correct whether or not the brick cache was refilled — so the
/// margin that path uses had never been measured against what a sculptor
/// actually sees. This measures it the way the drag is measured.
#[test]
fn appending_a_control_point_leaves_no_stale_bricks() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form) else {
        return;
    };
    let document = SharedDocument::new(document);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    if document
        .with(|d| geometry.rebuild(&harness.gpu, d))
        .is_err()
    {
        return;
    }
    let mut camera = Camera::default();
    match SculptModel::bounds(&document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }

    document.with(|d| d.begin_curve());
    for step in 0..20 {
        let t = step as f32 / 20.0;
        document
            .with(|d| {
                d.add_curve_point(
                    [
                        -1.3 + t * 2.6,
                        1.35 + (t * 7.0).sin() * 0.45,
                        (t * 5.0).cos() * 0.25,
                    ],
                    0.09,
                )
            })
            .expect("refused");
    }
    let _ = document.with(|d| geometry.sync(&harness.gpu, d));
    let appended = harness.capture(geometry.mesh(), &camera, false, "curve-append-incremental");

    let join = document.with(|d| d.curve().join);
    let other = if join == CurveJoin::Corners {
        CurveJoin::Through
    } else {
        CurveJoin::Corners
    };
    document.with(|d| d.set_curve_join(other)).expect("away");
    document.with(|d| d.set_curve_join(join)).expect("back");
    let _ = document.with(|d| geometry.sync(&harness.gpu, d));
    let whole = harness.capture(geometry.mesh(), &camera, false, "curve-append-refilled");

    let differing = appended
        .pixels
        .chunks_exact(4)
        .zip(whole.pixels.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 8))
        .count();
    println!("appending left {differing} pixels differing from a full refill");
    assert!(
        differing < 40,
        "{differing} pixels of the drawn surface change when every brick is \
         refilled, so laying the curve point by point left bricks stale. See \
         target/visual/curve-append-incremental.png"
    );
}
