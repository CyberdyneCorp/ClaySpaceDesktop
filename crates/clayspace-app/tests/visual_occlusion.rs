//! Ambient occlusion: the surface darkens where it closes in on itself.
//!
//! A MatCap is indexed by the view-space normal alone, so two points sharing a
//! normal shade identically whether one sits on an open flank or at the bottom
//! of a fold. The form is there and the detail is not, which is what makes an
//! unlit sculpt read as a blob.
//!
//! The obvious cheap answer was tried first and does not work here: darkening
//! by the divergence of the *normal* across a pixel quad. The reference form is
//! about seven triangles per covered pixel, so the interpolated normal field is
//! piecewise-linear below the scale a screen derivative measures, and what the
//! derivative reports is where the triangle edges are. Turned up far enough to
//! see, it drew the mesh. `docs/roadmap.md` keeps the picture.
//!
//! Depth does not have that problem. Positions are shared across a triangle
//! edge, so the depth buffer is a continuous function of screen position
//! however finely the surface is tessellated — which is why the pass reads
//! depth and derives its normal from that rather than taking the one the
//! vertex carries.
//!
//! The comparison is against the same frame with the passes switched off,
//! because there is nothing else to compare it to: the occlusion is computed
//! from the frame's own depth, so no second scene exists that should look like
//! it.

mod support;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_view::{Camera, Image};
use support::{save, Harness};

/// The mean of a frame's luminance over the pixels that are not the ground.
///
/// The ground is not written by the surface pipeline and so has no depth for
/// the occlusion pass to read; it is excluded rather than allowed to dilute
/// the difference the sculpt shows.
fn surface_mean(image: &Image, ground: [u8; 4]) -> f64 {
    let mut total = 0.0;
    let mut count = 0.0f64;
    for y in 0..image.height {
        for x in 0..image.width {
            let p = image.pixel(x, y);
            if p[0].abs_diff(ground[0]) < 6 && p[1].abs_diff(ground[1]) < 6 {
                continue;
            }
            total += p[0] as f64;
            count += 1.0;
        }
    }
    total / count.max(1.0)
}

/// A form with folds in it. A bare sphere is convex everywhere and would show
/// nothing however well the pass worked.
fn worked_form(harness: &Harness, geometry: &mut SurfaceGeometry) -> bool {
    let Ok(policy) = BackendPolicy::discover(None) else {
        return false;
    };
    let Ok(mut document) = Scene::Reference.build(policy) else {
        return false;
    };
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("rebuild");
    true
}

fn camera() -> Camera {
    // Close enough that the bands fill the frame, and turned off-axis so the
    // folds are seen along their length rather than end-on.
    let mut camera = Camera {
        distance: 2.6,
        ..Camera::default()
    };
    camera.orbit(0.5, -0.35);
    camera
}

#[test]
fn the_folds_of_a_worked_form_darken() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    if !worked_form(&harness, &mut geometry) {
        return;
    }
    let ground = harness.background();
    let camera = camera();

    harness.renderer.set_occlusion(false);
    let without = harness.target.capture(
        &harness.gpu,
        &harness.renderer,
        &camera,
        geometry.mesh(),
        false,
    );
    save(&without, "90-occlusion-off");

    harness.renderer.set_occlusion(true);
    let with = harness.target.capture(
        &harness.gpu,
        &harness.renderer,
        &camera,
        geometry.mesh(),
        false,
    );
    save(&with, "90-occlusion-on");

    let (dark, light) = with
        .pixels
        .chunks_exact(4)
        .zip(without.pixels.chunks_exact(4))
        .fold((0usize, 0usize), |(dark, light), (a, b)| {
            match a[0].cmp(&b[0]) {
                std::cmp::Ordering::Less => (dark + 1, light),
                std::cmp::Ordering::Greater => (dark, light + 1),
                std::cmp::Ordering::Equal => (dark, light),
            }
        });
    let before = surface_mean(&without, ground);
    let after = surface_mean(&with, ground);
    println!("surface mean {before:.1} -> {after:.1}; {dark} pixels darker, {light} lighter");

    // It has to do something, or the passes are running and producing white.
    assert!(
        after < before,
        "occlusion left the surface no darker ({before:.1} against {after:.1})"
    );

    // And it may only darken. The composite multiplies, so a lighter pixel
    // means the occlusion term came back above one — a sign flip, or a read
    // outside the texture answering something other than "unoccluded".
    assert_eq!(
        light, 0,
        "{light} pixels came out lighter with occlusion on, which a multiply \
         by a value in [0, 1] cannot do"
    );

    // Not so much that it stops being shading and starts being paint. A
    // sculptor reads form from the material, and the MatCap is what carries it.
    assert!(
        after > before * 0.75,
        "occlusion took the surface from {before:.1} to {after:.1}, which is \
         more than a quarter of the frame's light"
    );
}

#[test]
fn a_convex_form_is_barely_touched() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = clayspace_engine::ClayDocument::new(policy)
        .and_then(clayspace_engine::ClayDocument::with_starting_form)
    else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("rebuild");
    let ground = harness.background();
    let camera = Camera::default();

    harness.renderer.set_occlusion(false);
    let without = harness.target.capture(
        &harness.gpu,
        &harness.renderer,
        &camera,
        geometry.mesh(),
        false,
    );
    harness.renderer.set_occlusion(true);
    let with = harness.target.capture(
        &harness.gpu,
        &harness.renderer,
        &camera,
        geometry.mesh(),
        false,
    );

    let before = surface_mean(&without, ground);
    let after = surface_mean(&with, ground);
    println!("a bare sphere: surface mean {before:.1} -> {after:.1}");

    // Nothing on a sphere occludes anything else on it. A pass that darkened
    // it would be reporting its own sampling error as shape — which is what
    // the normal-derivative version did, and how it was caught.
    assert!(
        after > before * 0.97,
        "a convex form lost {:.1}% of its light to occlusion, which it has \
         nothing to occlude itself with",
        100.0 * (1.0 - after / before)
    );
}
