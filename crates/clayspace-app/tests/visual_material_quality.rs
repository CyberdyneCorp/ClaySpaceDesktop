//! What the material does beyond looking up a normal.
//!
//! Three additions live here, and each is off or subtle by default because the
//! default sculpt material is a decision that was already made and tested:
//!
//! - **mipmaps** on the MatCap, so a subtool small enough that its normals
//!   vary by more than a texel between neighbouring pixels stops sampling the
//!   material at random;
//! - the **contour**, an adjustable darkening toward the silhouette;
//! - the **cavity**, a screen-space curvature term that sharpens creases finer
//!   than the occlusion radius.
//!
//! The captures are the point as much as the assertions. A term that is subtle
//! by design cannot be judged by a number alone, and these write both states of
//! each into `target/visual/` so a person can look.

mod support;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_view::{Camera, Image, Renderer};
use support::{save, Harness};

/// The mean of a frame's luminance over the pixels that are not the ground.
fn surface_mean(image: &Image, ground: [u8; 4]) -> f64 {
    let mut total = 0.0;
    let mut count = 0.0f64;
    for y in 0..image.height {
        for x in 0..image.width {
            let p = image.pixel(x, y);
            if (0..3).all(|c| p[c].abs_diff(ground[c]) < 6) {
                continue;
            }
            total += p[0] as f64;
            count += 1.0;
        }
    }
    total / count.max(1.0)
}

/// How many surface pixels came out more than `levels` darker.
fn darkened(a: &Image, b: &Image, ground: [u8; 4], levels: i32) -> usize {
    let mut count = 0;
    for y in 0..a.height {
        for x in 0..a.width {
            let p = a.pixel(x, y);
            if (0..3).all(|c| p[c].abs_diff(ground[c]) < 6) {
                continue;
            }
            if p[0] as i32 - b.pixel(x, y)[0] as i32 > levels {
                count += 1;
            }
        }
    }
    count
}

/// The worked reference form, framed whole.
///
/// Whole, rather than the close crop the occlusion fixtures use. Occlusion is
/// judged on the folds in the middle of the form; the contour is judged on its
/// outline, and a camera close enough to fill the frame pushes most of that
/// outline off the edge of it.
fn worked(harness: &Harness) -> Option<(SurfaceGeometry, Camera)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = Scene::Reference.build(policy).ok()?;
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, &mut document).ok()?;
    let mut camera = support::framed(&document);
    camera.orbit(0.5, -0.35);
    Some((geometry, camera))
}

/// The contour darkens the silhouette and leaves the middle of the form alone.
///
/// Both halves matter. A term that darkened everything would be a tint on the
/// material — the sculptor asked for the *contour* to read, not for the clay to
/// change colour — and one that darkened nothing would not be doing anything.
#[test]
fn the_contour_darkens_the_silhouette_and_not_the_form() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };
    let ground = harness.background();

    harness.renderer.set_contour(0.0);
    let plain = harness.capture(geometry.mesh(), &camera, false, "96-contour-off");
    harness.renderer.set_contour(0.5);
    let contoured = harness.capture(geometry.mesh(), &camera, false, "96-contour-on");
    harness.renderer.set_contour(0.0);

    let darker = darkened(&plain, &contoured, ground, 8);
    let before = surface_mean(&plain, ground);
    let after = surface_mean(&contoured, ground);
    println!("contour: {darker} pixels darker, surface mean {before:.1} -> {after:.1}");

    assert!(
        darker > 200,
        "the contour darkened only {darker} pixels, which is not a silhouette \
         reading — see target/visual/96-contour-on.png"
    );
    // The band is a fraction of the form, not the form. Measured on the
    // reference at strength 0.5 it is well under a fifth of the covered
    // pixels; a term that took half of them would be a tint.
    let covered = (0..plain.height)
        .flat_map(|y| (0..plain.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let p = plain.pixel(*x, *y);
            !(0..3).all(|c| p[c].abs_diff(ground[c]) < 6)
        })
        .count();
    assert!(
        darker * 3 < covered,
        "{darker} of {covered} surface pixels darkened, which is the whole \
         form and not its contour"
    );
    assert!(
        after < before,
        "the contour did not darken the form at all ({before:.1} -> {after:.1})"
    );
}

/// The cavity term sharpens creases that occlusion alone leaves flat.
///
/// Compared against the same frame with the term at zero and occlusion on in
/// both, so what is being measured is the term rather than the shading it sits
/// on top of. It runs only at the highest quality tier, which is what a
/// renderer nobody has told anything to is already at.
#[test]
fn the_cavity_term_sharpens_creases_occlusion_leaves_flat() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };
    let ground = harness.background();

    harness.renderer.set_cavity(0.0);
    let without = harness.capture(geometry.mesh(), &camera, false, "96-cavity-off");
    harness.renderer.set_cavity(Renderer::CAVITY);
    let with = harness.capture(geometry.mesh(), &camera, false, "96-cavity-on");

    let darker = darkened(&without, &with, ground, 4);
    let lighter = darkened(&with, &without, ground, 4);
    let before = surface_mean(&without, ground);
    let after = surface_mean(&with, ground);
    println!("cavity: {darker} darker, {lighter} lighter, mean {before:.1} -> {after:.1}");

    assert!(
        darker > 100,
        "the cavity term darkened only {darker} pixels — see \
         target/visual/96-cavity-on.png"
    );
    assert_eq!(
        lighter, 0,
        "{lighter} pixels came out lighter, which a multiply by a value in \
         [0, 1] cannot do"
    );
    // Subtle, or it stops being shading and becomes an ink line along every
    // crease — which is a drawing of the mesh, not a picture of the form.
    assert!(
        after > before * 0.9,
        "the cavity term took the surface from {before:.1} to {after:.1}, \
         which is a tenth of the frame's light for a detail term"
    );
}

/// Turning the term off puts the frame back exactly where it was.
///
/// Cheap to state and worth stating: both of these are display parameters, and
/// a display parameter that leaves a residue behind when it is switched off is
/// a bug that only shows up two settings later.
#[test]
fn the_material_parameters_leave_no_residue() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };

    harness.renderer.set_cavity(0.0);
    harness.renderer.set_contour(0.0);
    let before = harness.capture(geometry.mesh(), &camera, false, "96-plain-before");

    harness.renderer.set_contour(0.8);
    harness.renderer.set_cavity(1.0);
    let _ = harness.capture(geometry.mesh(), &camera, false, "96-plain-turned-up");

    harness.renderer.set_contour(0.0);
    harness.renderer.set_cavity(0.0);
    let after = harness.capture(geometry.mesh(), &camera, false, "96-plain-after");

    let changed = support::differing_pixels(&before, &after);
    save(&after, "96-plain-after");
    assert_eq!(
        changed, 0,
        "{changed} pixels differ between the frame before the parameters were \
         turned up and the frame after they were turned back down"
    );
}
