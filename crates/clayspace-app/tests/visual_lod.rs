//! What the coarse surface actually looks like, and from how far away.
//!
//! `lod_switching.rs` asserts that dropping to the mips changes the triangle
//! count and that approaching brings the full surface back. Neither says
//! whether the result is *worth looking at*, which is the only question the
//! policy's thresholds are really about: 3.0 extents to drop and 2.5 to
//! restore were chosen on the claim that at that distance a mip's doubled
//! spacing falls under a pixel.
//!
//! So this captures the same model both ways, twice — filling the frame, where
//! the difference should be plain, and at the distance the policy actually
//! drops at, where it should be hard to find. A threshold that turns out to be
//! too near shows up here as a coarse silhouette rather than as a number
//! nobody questions.
//!
//! Two things in the coarse capture are expected, and were each chased down
//! once already rather than assumed:
//!
//! - **The speckle.** Small dark specks across the surface. They are not holes
//!   and not an LOD defect: the same document meshed at level 0 with face
//!   normals has them too, and with gradient normals it has none. They are
//!   degenerate triangles — 316 of 86130 here — shading to a garbage normal
//!   when the normal comes from the triangle rather than from the field. The
//!   coarse surface cannot escape them because level 1 refuses gradients, and
//!   the same specks are already on screen during every drag, where `refine`
//!   clears them on pointer-up.
//! - **Missing coarse blocks.** A mip needs all eight children evaluated, and
//!   the cache only evaluates surface bricks, so a coarse block on the edge of
//!   the surface band never gets one however long it settles — 70 of 242 here,
//!   covering 184 of 1043 fine bricks. Meshing those at level 0 and splicing
//!   them in was measured: it moves 0.69% of the frame, against 2.09% between
//!   the coarse surface and the full one. Not taken, because mixing levels in
//!   one surface risks cracks where the spacings meet.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_lod
//! open target/visual
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Detail, DetailPolicy, GestureSample, SculptModel, ToolKind};
use clayspace_view::{Camera, Image};
use support::Harness;

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// A settled surface with its mips built — what the end of a gesture leaves.
fn settled() -> Option<ClayDocument> {
    let mut document = document()?;
    for step in 0..8 {
        let t = step as f32 / 7.0;
        let x = -0.4 + t * 0.8;
        let y = -0.25 + t * 0.45;
        let z = (1.0 - x * x - y * y).max(0.05).sqrt();
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &[GestureSample {
                    position: [x, y, z],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("a dab");
    }
    document.build_mips().expect("build the mips");
    Some(document)
}

/// The camera framing the model, and how wide the model is.
fn framed(document: &ClayDocument) -> (Camera, f32) {
    let mut camera = Camera::default();
    let extent = match SculptModel::bounds(document) {
        Some((min, max)) => {
            camera.frame_bounds(min.into(), max.into());
            (0..3).fold(0.0f32, |widest, axis| widest.max(max[axis] - min[axis]))
        }
        None => {
            camera.frame_default();
            1.0
        }
    };
    (camera, extent)
}

/// The share of pixels that differ at all, and the worst channel gap.
fn compare(a: &Image, b: &Image) -> (f64, u8) {
    let mut differing = 0usize;
    let mut worst = 0u8;
    for y in 0..a.height.min(b.height) {
        for x in 0..a.width.min(b.width) {
            let (pa, pb) = (a.pixel(x, y), b.pixel(x, y));
            let gap = (0..3).map(|c| pa[c].abs_diff(pb[c])).max().unwrap_or(0);
            if gap > 8 {
                differing += 1;
                worst = worst.max(gap);
            }
        }
    }
    let total = (a.width * a.height) as f64;
    (differing as f64 / total, worst)
}

#[test]
fn the_coarse_surface_is_worth_looking_at() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = settled() else {
        return;
    };
    if document
        .drawable_coarse_keys()
        .map(|keys| keys.is_empty())
        .unwrap_or(true)
    {
        return;
    }

    let (near, extent) = framed(&document);
    let policy = DetailPolicy::default();

    // The same camera pushed back to where the policy would drop detail. The
    // framed camera is the near one; `drop_beyond` is in extents, so the far
    // distance is that many model widths from the target.
    let mut far = near;
    far.distance = policy.drop_beyond * extent;

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the full surface");
    assert_eq!(geometry.detail(), Detail::Full);
    let full_triangles = geometry.triangle_count();
    let full_near = harness.capture(geometry.mesh(), &near, false, "18-lod-full-near");
    let full_far = harness.capture(geometry.mesh(), &far, false, "18-lod-full-far");

    geometry
        .set_detail(&harness.gpu, &mut document, Detail::Reduced)
        .expect("drop to the mips");
    assert_eq!(
        geometry.detail(),
        Detail::Reduced,
        "the coarse surface was available and was not taken"
    );
    let coarse_triangles = geometry.triangle_count();
    let coarse_near = harness.capture(geometry.mesh(), &near, false, "18-lod-coarse-near");
    let coarse_far = harness.capture(geometry.mesh(), &far, false, "18-lod-coarse-far");

    // Something was drawn both ways. A coarse surface that rendered to an
    // empty frame would satisfy every triangle-count assertion in the suite.
    let background = harness.background();
    for (image, name) in [(&coarse_near, "near"), (&coarse_far, "far")] {
        let lit = (0..image.height)
            .flat_map(|y| (0..image.width).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let pixel = image.pixel(*x, *y);
                (0..3).any(|c| pixel[c].abs_diff(background[c]) > 8)
            })
            .count();
        assert!(
            lit > 0,
            "the coarse surface drew an empty frame at {name} distance"
        );
    }

    let (near_share, _) = compare(&full_near, &coarse_near);
    let (far_share, _) = compare(&full_far, &coarse_far);

    // The point of the whole feature, and the point of the threshold. Close
    // up the mip is visibly coarser; at the distance the policy drops at, far
    // less of the frame can tell. Stated as a comparison between the two
    // rather than as an absolute share, because an absolute one would be a
    // golden that every driver argues with.
    assert!(
        far_share < near_share,
        "the coarse surface was no less distinguishable at {} extents than \
         filling the frame: {far_share:.4} against {near_share:.4}. The drop \
         threshold is too near, or the mip is not being drawn at the distance \
         it was chosen for",
        policy.drop_beyond
    );

    assert!(
        coarse_triangles < full_triangles,
        "the coarse surface was not coarser: {coarse_triangles} triangles \
         against {full_triangles}"
    );

    // Recorded rather than asserted on: what the drop actually buys, in the
    // units the policy is trading against.
    println!(
        "triangles {full_triangles} -> {coarse_triangles} \
         ({:.0}%), frame differing near {near_share:.4} far {far_share:.4}",
        100.0 * coarse_triangles as f64 / full_triangles as f64
    );
}
