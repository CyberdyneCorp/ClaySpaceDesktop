//! Can a sculptor see the line the tube is following?
//!
//! Two things were wrong at once, and each hid the other. The overlay drew the
//! *control polygon* — straight chords between the points — which under
//! `Through` or `Rounded` is a different line from the one the tube is swept
//! along. And the surface stayed opaque while a curve was up, so the scaffold
//! shader dimmed whatever the tube stood in front of, which on a guide running
//! down the middle of its own tube is the whole of it.
//!
//! So the one line that could be seen was faint, and it was the wrong line.
//!
//! ```sh
//! cargo test -p clayspace-app --release --test visual_curve_guide -- --nocapture
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{CurveJoin, CurveModel};
use clayspace_view::Camera;
use support::Harness;

/// A curve bent hard enough that its chords and its path are different lines.
fn bend(document: &mut ClayDocument) {
    document.begin_curve();
    for at in [
        [-1.4f32, 0.5, 0.0],
        [-0.5, 1.9, 0.0],
        [0.5, 0.2, 0.0],
        [1.4, 1.6, 0.0],
    ] {
        document.add_curve_point(at, 0.16).expect("refused");
    }
    document.set_curve_join(CurveJoin::Through).expect("join");
}

#[test]
fn the_guide_is_visible_through_the_tube_it_runs_inside() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };
    let mut camera = Camera::default();
    camera.frame_bounds([-2.0, -2.0, -2.0].into(), [2.0, 2.0, 2.0].into());
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    bend(&mut document);
    geometry.sync(&harness.gpu, &mut document).expect("mesh");

    let curve = document.curve();
    let points: Vec<[f32; 3]> = curve.points.iter().map(|p| p.position).collect();
    let guide = curve.path();

    // Ghosted for BOTH captures, and the handles drawn in both.
    //
    // The first version of this varied the ghosting too, and ghosting alone
    // repaints the whole tube — thirteen thousand pixels of difference before
    // a single line is drawn. The assertion passed and would have passed with
    // no guide at all, which is the third time today a count has been true of
    // the wrong thing. Holding everything else fixed makes the difference
    // below the guide and nothing else.
    harness.renderer.set_ghosted(true);
    harness.renderer.set_lattice(
        &harness.gpu,
        clayspace_view::LatticeView {
            points: &points,
            edges: &[],
            guide: &[],
            selected: &[],
            gizmo: None,
            outline: None,
            subtool_outline: None,
            handle: 0.06,
        },
    );
    let bare = harness.capture(geometry.mesh(), &camera, false, "curve-guide-none");

    // And now with the guide, which is the only thing that changes.
    harness.renderer.set_ghosted(true);
    harness.renderer.set_lattice(
        &harness.gpu,
        clayspace_view::LatticeView {
            points: &points,
            edges: &[],
            guide: &guide,
            selected: &[],
            gizmo: None,
            outline: None,
            subtool_outline: None,
            handle: 0.06,
        },
    );
    let shown = harness.capture(geometry.mesh(), &camera, false, "curve-guide-drawn");

    // Counted only where the tube already stood.
    //
    // A guide that is drawn but invisible where it matters is the defect this
    // change exists to fix — the line runs down the middle of its own tube, so
    // "the overlay drew something" is not the question. Pixels the guide
    // changed *against the surface* are, and they are the ones the ghosting
    // and the scaffold pass have to get right together.
    let background = harness.background();
    let is_background = |p: &[u8]| (0..3).all(|c| p[c].abs_diff(background[c]) <= 6);
    let (mut over_tube, mut anywhere) = (0usize, 0usize);
    for (before, after) in bare
        .pixels
        .chunks_exact(4)
        .zip(shown.pixels.chunks_exact(4))
    {
        if (0..3).any(|c| before[c].abs_diff(after[c]) > 12) {
            anywhere += 1;
            if !is_background(before) {
                over_tube += 1;
            }
        }
    }
    // How far the guide's pixels actually move, which is what legibility is.
    let mut strength = 0u64;
    for (before, after) in bare
        .pixels
        .chunks_exact(4)
        .zip(shown.pixels.chunks_exact(4))
    {
        if (0..3).any(|c| before[c].abs_diff(after[c]) > 12) && !is_background(before) {
            strength += (0..3)
                .map(|c| u64::from(before[c].abs_diff(after[c])))
                .sum::<u64>()
                / 3;
        }
    }
    let contrast = strength as f64 / over_tube.max(1) as f64;
    println!(
        "the guide changed {anywhere} pixels, {over_tube} over the tube, \
         mean contrast {contrast:.1}"
    );
    assert!(
        over_tube > 100,
        "only {over_tube} of the guide's {anywhere} pixels land on the tube \
         itself, so the part of the line that runs inside the form is not \
         reading. See target/visual/curve-guide-drawn.png"
    );

    // Bright enough to follow, and this is the half that holds the ghosting.
    //
    // Counting pixels is not enough and was checked: with the surface left
    // opaque the guide still moves 232 pixels — the scaffold shader dims it
    // rather than hiding it, so a count clears any threshold either way. What
    // separates the two is how far those pixels move. Measured on this frame,
    // 54.6 with the surface ghosted and 17.8 without it, so the guide is
    // roughly three times as legible against the tube it runs inside.
    assert!(
        contrast > 35.0,
        "the guide's pixels move by only {contrast:.1} against the tube, \
         where a ghosted surface gives about 55 and an opaque one about 18. \
         The line is being drawn and dimmed rather than drawn and seen"
    );
}
