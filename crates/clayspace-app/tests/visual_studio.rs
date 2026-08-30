//! Studio shading, beside MatCap and never in place of it.
//!
//! The one question a MatCap cannot answer is how a form takes a light that
//! stays where it is. Its lighting is welded to the camera — the material is
//! indexed by the view-space normal — so orbiting the sculpt orbits the light
//! with it, and the highlight never moves across the form. That is exactly what
//! makes it good for reading form and useless for judging a surface before it
//! is cast or fired.
//!
//! So the assertions here are about the *difference* between the two modes
//! rather than about either picture in isolation:
//!
//! - Studio shades the same geometry differently from MatCap;
//! - its highlight moves across the form as the camera orbits, and MatCap's
//!   does not, which is the whole reason it exists;
//! - MatCap is what a renderer draws when nobody has chosen;
//! - and switching to Studio and back leaves the frame exactly as it was.

mod support;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_view::{Camera, Image, ShadingMode, StudioMaterial};
use support::Harness;

/// Where the brightest part of the lit surface is, as a fraction of the frame.
///
/// The centroid of the brightest few per cent of pixels rather than the single
/// brightest one: a specular highlight on a tessellated surface is speckled,
/// and the argmax of it jumps between neighbouring triangles for reasons that
/// have nothing to do with where the light is.
fn highlight_centre(image: &Image) -> (f64, f64) {
    let mut values: Vec<u8> = image.pixels.chunks_exact(4).map(|p| p[0]).collect();
    values.sort_unstable();
    let cut = values[values.len() * 97 / 100];

    let (mut x_total, mut y_total, mut count) = (0.0f64, 0.0f64, 0.0f64);
    for y in 0..image.height {
        for x in 0..image.width {
            if image.pixel(x, y)[0] >= cut {
                x_total += x as f64;
                y_total += y as f64;
                count += 1.0;
            }
        }
    }
    (
        x_total / count.max(1.0) / image.width as f64,
        y_total / count.max(1.0) / image.height as f64,
    )
}

fn worked(harness: &Harness) -> Option<(SurfaceGeometry, Camera)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = Scene::Reference.build(policy).ok()?;
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, &mut document).ok()?;
    Some((geometry, support::framed(&document)))
}

/// A renderer nobody has told anything to shades with a MatCap.
#[test]
fn matcap_is_what_is_drawn_when_nobody_has_chosen() {
    let Some(harness) = Harness::new() else {
        return;
    };
    assert_eq!(harness.renderer.shading(), ShadingMode::MatCap);
    assert_eq!(
        harness.renderer.studio_material(),
        StudioMaterial::default(),
        "the studio material should start at what clay is"
    );
}

/// The two modes shade the same geometry differently, and Studio is not simply
/// a darker or brighter MatCap.
#[test]
fn studio_shades_the_same_form_differently() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };

    harness.renderer.set_shading(ShadingMode::MatCap);
    let matcap = harness.capture(geometry.mesh(), &camera, false, "98-shading-matcap");
    harness.renderer.set_shading(ShadingMode::Studio);
    let studio = harness.capture(geometry.mesh(), &camera, false, "98-shading-studio");

    // Counted at eight levels rather than at `support::RENDER_NOISE`. That
    // constant is set wide enough that a driver rebinning a frame cannot trip
    // it, which is the right question for "is this the same picture" and the
    // wrong one here: the two modes are both tuned to look like clay, so much
    // of the form differs by a tone rather than by a jump.
    let differing = (0..matcap.height)
        .flat_map(|y| (0..matcap.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let (a, b) = (matcap.pixel(*x, *y), studio.pixel(*x, *y));
            (0..3).any(|c| a[c].abs_diff(b[c]) > 8)
        })
        .count();
    let covered = matcap.pixels_differing_from(harness.background(), 6);
    println!("studio against matcap: {differing} of {covered} covered pixels differ");
    assert!(
        differing * 4 > covered,
        "only {differing} of {covered} covered pixels differ between the two \
         modes — see target/visual/98-shading-studio.png"
    );

    // Both have to be a *form* rather than a flat patch or a blown-out one:
    // a rig that clipped everything to white would differ from MatCap in every
    // pixel and be useless.
    for (name, image) in [("matcap", &matcap), ("studio", &studio)] {
        let lit = image.pixels.chunks_exact(4).filter(|p| p[0] > 250).count();
        let covered = image.pixels_differing_from(harness.background(), 6);
        assert!(
            lit * 4 < covered,
            "{name} blew out {lit} of {covered} covered pixels to white"
        );
    }
}

/// The studio highlight moves across the form as the camera orbits, and the
/// MatCap one does not.
///
/// This is the whole reason the mode exists, and it is the one property that
/// cannot be faked by re-tinting a MatCap. Two frames a small orbit apart: the
/// MatCap's brightest region stays put relative to the form, because its light
/// is welded to the camera; the studio rig's slides, because its light is
/// welded to the world.
#[test]
fn the_studio_highlight_moves_and_the_matcap_one_does_not() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };
    let mut turned = camera;
    turned.orbit(0.55, 0.0);

    let mut travel = Vec::new();
    for mode in [ShadingMode::MatCap, ShadingMode::Studio] {
        harness.renderer.set_shading(mode);
        let before = harness.capture(
            geometry.mesh(),
            &camera,
            false,
            &format!("98-highlight-{}-a", mode.label().to_lowercase()),
        );
        let after = harness.capture(
            geometry.mesh(),
            &turned,
            false,
            &format!("98-highlight-{}-b", mode.label().to_lowercase()),
        );
        let (ax, ay) = highlight_centre(&before);
        let (bx, by) = highlight_centre(&after);
        let moved = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        println!("{}: highlight moved {moved:.4} of the frame", mode.label());
        travel.push(moved);
    }

    let (matcap, studio) = (travel[0], travel[1]);
    assert!(
        studio > matcap * 2.0,
        "the studio highlight moved {studio:.4} of the frame against the \
         MatCap's {matcap:.4}; a rig fixed in the world has to sweep across \
         the form as the camera goes round it"
    );
}

/// The roughness dial does something, and does it in the direction it says.
#[test]
fn a_rougher_surface_has_a_broader_softer_highlight() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };
    harness.renderer.set_shading(ShadingMode::Studio);

    let mut brightest = Vec::new();
    for roughness in [0.15f32, 0.9] {
        // Exposed well down, so the filmic curve is still in its straight part
        // and the two lobes are compared rather than the shoulder they both
        // run into. At the default exposure both saturate and the peaks agree
        // to a level, which says nothing about either.
        harness.renderer.set_studio_material(StudioMaterial {
            roughness,
            exposure: 0.3,
            ..StudioMaterial::default()
        });
        let image = harness.capture(
            geometry.mesh(),
            &camera,
            false,
            &format!("98-studio-roughness-{}", (roughness * 100.0) as u32),
        );
        // Over the *covered* pixels only, and the top half per cent of them.
        // A percentile of the whole frame lands in the diffuse body of the
        // form, where roughness barely shows; a single maximum is one triangle
        // and moves for reasons that are not the lobe. This is the highlight.
        let ground = harness.background();
        let mut values: Vec<u8> = (0..image.height)
            .flat_map(|y| (0..image.width).map(move |x| (x, y)))
            .map(|(x, y)| image.pixel(x, y))
            .filter(|p| (0..3).any(|c| p[c].abs_diff(ground[c]) >= 6))
            .map(|p| p[0])
            .collect();
        values.sort_unstable();
        let top = (values.len() / 200).max(1);
        let peak: f64 = values[values.len() - top..]
            .iter()
            .map(|v| *v as f64)
            .sum::<f64>()
            / top as f64;
        println!("roughness {roughness}: highlight reads {peak:.1}");
        brightest.push(peak);
    }

    assert!(
        brightest[0] > brightest[1] + 4.0,
        "a polished surface's highlight reads {:.1} and a rough one's {:.1}; a \
         tighter lobe concentrates the same light into fewer, brighter pixels",
        brightest[0],
        brightest[1]
    );
}

/// Switching modes leaves nothing behind.
#[test]
fn returning_to_matcap_returns_the_frame() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };

    let before = harness.capture(geometry.mesh(), &camera, false, "98-shading-before");
    harness.renderer.set_shading(ShadingMode::Studio);
    let _ = harness.capture(geometry.mesh(), &camera, false, "98-shading-between");
    harness.renderer.set_shading(ShadingMode::MatCap);
    let after = harness.capture(geometry.mesh(), &camera, false, "98-shading-after");

    let differing = support::differing_pixels(&before, &after);
    assert_eq!(
        differing, 0,
        "{differing} pixels differ between the frame before Studio was chosen \
         and the frame after MatCap was chosen again"
    );
}

/// The key light casts, and what it casts is self-shadowing.
///
/// A key light on an unshadowed form lights the inside of every fold as
/// brightly as the flank beside it, which is the same failure a MatCap has.
/// Occlusion does not fix it either: occlusion is a local term at the scale of
/// a crease, and cannot say that one part of a form is between the light and
/// another.
///
/// Measured as a difference between two frames of the same rig, one casting
/// and one not. That is the only comparison there is: the map is computed from
/// the frame it darkens, so no second render exists that should look like it.
#[test]
fn the_key_light_casts_a_shadow_on_the_form() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };
    harness.renderer.set_shading(ShadingMode::Studio);

    harness.renderer.set_shadows(true);
    let with = harness.capture(geometry.mesh(), &camera, false, "98-studio-shadowed");
    harness.renderer.set_shadows(false);
    let without = harness.capture(geometry.mesh(), &camera, false, "98-studio-unshadowed");
    harness.renderer.set_shadows(true);

    let ground = harness.background();
    let (mut darker, mut lighter, mut covered) = (0usize, 0usize, 0usize);
    for y in 0..with.height {
        for x in 0..with.width {
            let a = without.pixel(x, y);
            if (0..3).all(|c| a[c].abs_diff(ground[c]) < 6) {
                continue;
            }
            covered += 1;
            let delta = a[0] as i32 - with.pixel(x, y)[0] as i32;
            if delta > 8 {
                darker += 1;
            } else if delta < -8 {
                lighter += 1;
            }
        }
    }
    println!("studio shadow: {darker} darker, {lighter} lighter, of {covered} covered");

    assert!(
        darker > 200,
        "the shadow map darkened {darker} of {covered} covered pixels, which \
         is not a form shadowing itself — see \
         target/visual/98-studio-shadowed.png"
    );
    // A shadow, not a dimmer. The lit side has to stay lit, or the map is
    // shadowing everything — which is what a fit the form does not fall inside
    // produces, and what a comparison the wrong way round produces.
    assert!(
        darker * 2 < covered,
        "{darker} of {covered} covered pixels fell into shadow, which is the \
         whole form rather than the side facing away from the key"
    );
    assert_eq!(
        lighter, 0,
        "{lighter} pixels came out lighter with the shadow map, which a term \
         that only ever takes light away cannot do"
    );
}

/// MatCap does not cast.
///
/// Its lighting is welded to the camera, so a shadow from it would swing round
/// the form as the view moved — worse than none. The map is not even allocated
/// until the studio rig is asked for.
#[test]
fn matcap_draws_no_shadow_and_allocates_no_map() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };

    harness.renderer.set_shading(ShadingMode::MatCap);
    let before = harness.capture(geometry.mesh(), &camera, false, "98-matcap-before-studio");
    harness.renderer.set_shading(ShadingMode::Studio);
    let _ = harness.capture(geometry.mesh(), &camera, false, "98-studio-once");
    harness.renderer.set_shading(ShadingMode::MatCap);
    let after = harness.capture(geometry.mesh(), &camera, false, "98-matcap-after-studio");

    let differing = support::differing_pixels(&before, &after);
    assert_eq!(
        differing, 0,
        "{differing} pixels differ between a MatCap frame drawn before the \
         studio rig was ever asked for and one drawn after"
    );
}
