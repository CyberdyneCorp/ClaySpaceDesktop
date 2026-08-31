//! The scaffolding is drawn faint where the sculpt stands in front of it.
//!
//! A manipulator is drawn over the form whatever its depth, which is what
//! makes it grabbable — and drawn at one strength everywhere, a rotate ring
//! around a head reads as a circle painted on the frame rather than as a hoop
//! the head passes through. So the pass samples the depth the scene wrote and
//! draws faint where the sculpt is nearer, without binding a depth attachment
//! and without moving out of its place after the occlusion composite.

mod support;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_model::GizmoMode;
use clayspace_view::{Camera, GizmoView, Image, LatticeView};
use support::Harness;

fn worked(harness: &Harness) -> Option<(SurfaceGeometry, Camera)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = Scene::Reference.build(policy).ok()?;
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, &mut document).ok()?;
    Some((geometry, support::framed(&document)))
}

/// A rotate ring around the middle of the form.
///
/// Rotate rather than move: the guide's own complaint is about a ring, and a
/// ring is the case where it matters most — an arrow points away from the form
/// and half a ring is inside it.
fn ring_around_the_form(harness: &mut Harness) {
    let gpu = harness.gpu.clone();
    harness.renderer.set_lattice(
        &gpu,
        LatticeView {
            points: &[],
            edges: &[],
            selected: &[],
            gizmo: Some(GizmoView {
                pivot: [0.0, 0.0, 0.0],
                mode: GizmoMode::Rotate,
                reach: 1.4,
                hovered: None,
                view_axis: [0.0, 0.0, 1.0],
                per_axis_scale: false,
            }),
            outline: None,
            subtool_outline: None,
            handle: 0.06,
        },
    );
}

fn no_ring(harness: &mut Harness) {
    let gpu = harness.gpu.clone();
    harness.renderer.set_lattice(
        &gpu,
        LatticeView {
            points: &[],
            edges: &[],
            selected: &[],
            gizmo: None,
            outline: None,
            subtool_outline: None,
            handle: 0.06,
        },
    );
}

/// Which pixels the ring covers, as the difference against a frame without it.
fn ring_pixels(with: &Image, without: &Image) -> Vec<(u32, u32)> {
    (0..with.height)
        .flat_map(|y| (0..with.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let (a, b) = (with.pixel(*x, *y), without.pixel(*x, *y));
            (0..3).any(|c| a[c].abs_diff(b[c]) > 24)
        })
        .collect()
}

/// The brightest channel of a pixel.
fn light(image: &Image, at: (u32, u32)) -> u32 {
    let p = image.pixel(at.0, at.1);
    (0..3).map(|c| p[c] as u32).max().unwrap_or(0)
}

/// 8-bit sRGB to linear.
///
/// The target is `Rgba8UnormSrgb`, so the device blends in linear light and
/// stores the result encoded. Reading the alpha back out of a blended pixel
/// means undoing that first — done in sRGB the figures come out systematically
/// high, and the whole measurement here is a ratio of differences.
fn linear(value: u8) -> f64 {
    let c = value as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// How opaque the ring was drawn at one pixel, recovered from the frame.
///
/// The pass alpha-blends, so a drawn pixel is `a * ring + (1 - a) * under`.
/// Three frames give everything but `a`: the form alone is `under`, the ring
/// over an empty scene is `ring` — nothing is in front of it there, so it is
/// drawn whole — and the frame with both is the blend. Solving for `a` measures
/// the dimming itself rather than a colour difference, which is what the first
/// version of this got wrong: it normalised the ring over the *form* against
/// the ring over the *background*, and those differ because the two backgrounds
/// differ, dimming or no dimming.
///
/// `None` where the ring's colour is too close to what is under it for the
/// division to say anything.
fn drawn_alpha(blended: &Image, under: &Image, whole: &Image, at: (u32, u32)) -> Option<f64> {
    let (r, u, w) = (
        blended.pixel(at.0, at.1),
        under.pixel(at.0, at.1),
        whole.pixel(at.0, at.1),
    );
    // The channel with the most room in it, so the ratio is least sensitive to
    // a level of rounding.
    let (channel, span) = (0..3)
        .map(|c| (c, (linear(w[c]) - linear(u[c])).abs()))
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    if span < 0.02 {
        return None;
    }
    Some(
        ((linear(r[channel]) - linear(u[channel])) / (linear(w[channel]) - linear(u[channel])))
            .clamp(0.0, 2.0),
    )
}

/// Whether the sculpt covers this pixel, read off the frame with no scaffolding.
fn on_form(bare: &Image, ground: [u8; 4], at: (u32, u32)) -> bool {
    let under = bare.pixel(at.0, at.1);
    (0..3).any(|c| under[c].abs_diff(ground[c]) >= 6)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// The alpha the ring was drawn with, at every pixel it covers over the form.
///
/// `whole` is the same ring over an empty scene, which is the same geometry from
/// the same camera and so covers the same pixels.
fn alphas_over_the_form(
    ringed: &Image,
    bare: &Image,
    whole: &Image,
    empty: &Image,
    ground: [u8; 4],
) -> Vec<f64> {
    ring_pixels(ringed, bare)
        .into_iter()
        .filter(|at| on_form(bare, ground, *at))
        // Only where the ring was actually drawn in the empty frame too, since
        // that frame is where its own colour is read from.
        .filter(|at| light(whole, *at).abs_diff(light(empty, *at)) > 24)
        .filter_map(|at| drawn_alpha(ringed, bare, whole, at))
        .collect()
}

/// A rotate ring around a solid form is drawn at two strengths: full where it
/// stands in front of the form, faint where the form stands in front of it.
///
/// Two populations rather than a mean, because the mean is the weaker claim.
/// Half of a ring crossing the form is in front of it and half behind, so a
/// mean over the crossing is a mean over both — and it would fall just as far
/// if the whole ring were dimmed a little, which is a tint and not a depth cue.
/// What the guide asks for is that the two halves differ.
#[test]
fn a_ring_is_faint_where_the_form_is_in_front_of_it() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };
    let ground = harness.background();
    let nothing = clayspace_view::GpuMesh::new(&harness.gpu);

    no_ring(&mut harness);
    let bare = harness.capture(geometry.mesh(), &camera, false, "9d-xray-bare");
    let empty = harness.capture(&nothing, &camera, false, "9d-xray-empty");
    ring_around_the_form(&mut harness);
    let ringed = harness.capture(geometry.mesh(), &camera, false, "9d-xray-ring");
    // The same ring with nothing in front of it, which is where its own colour
    // is read from.
    let whole = harness.capture(&nothing, &camera, false, "9d-xray-whole");

    let alphas = alphas_over_the_form(&ringed, &bare, &whole, &empty, ground);
    assert!(
        alphas.len() > 300,
        "the ring's alpha could be measured at only {} pixels over the form, \
         so this measures nothing — see target/visual/9d-xray-ring.png",
        alphas.len()
    );

    let full = alphas.iter().filter(|a| **a > 0.8).count();
    let faint = alphas.iter().filter(|a| **a < 0.6).count();
    println!(
        "over the form the ring was drawn at mean alpha {:.2}: {full} pixels \
         above 0.8 and {faint} below 0.6, of {}",
        mean(&alphas),
        alphas.len()
    );

    assert!(
        full > 50,
        "only {full} of {} pixels of the ring crossing the form were drawn \
         above alpha 0.8, so the whole ring has been dimmed rather than the \
         half that is behind",
        alphas.len()
    );
    assert!(
        faint > 50,
        "only {faint} of {} pixels of the ring crossing the form were drawn \
         below alpha 0.6, so the form standing in front of it changes nothing",
        alphas.len()
    );
}

/// The orientation gizmo is not dimmed by a sculpt filling its corner.
///
/// It draws in a corner viewport with a camera of its own, so the scene's depth
/// in those pixels is whatever a sculpt reaching into that corner happened to
/// write — it says nothing about where the gizmo is. The gizmo was freed from
/// exactly that once already, by being taken out of the depth-tested pass.
///
/// What holds it now is that its pipeline binds no depth at all, rather than a
/// flag saying not to dim: a flag that says "do not dim" is a flag that will one
/// day be set wrongly. This is the test that says the two pipelines have not
/// been mixed up.
#[test]
fn the_orientation_gizmo_is_not_dimmed_by_the_sculpt() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, _)) = worked(&harness) else {
        return;
    };
    let nothing = clayspace_view::GpuMesh::new(&harness.gpu);
    // Close enough that the form covers the corner the gizmo sits in.
    let mut camera = Camera {
        distance: 1.6,
        ..Camera::default()
    };
    camera.orbit(0.3, -0.2);
    let ground = harness.background();

    harness.renderer.show_gizmo = false;
    let bare = harness.capture(geometry.mesh(), &camera, false, "9d-xray-cube-bare");
    harness.renderer.show_gizmo = true;
    let over_form = harness.capture(geometry.mesh(), &camera, false, "9d-xray-cube");
    // The gizmo with nothing behind it, which is where its own colour is read
    // from — it is drawn whole there whatever this pass does.
    let whole = harness.capture(&nothing, &camera, false, "9d-xray-cube-whole");
    let empty = {
        harness.renderer.show_gizmo = false;
        harness.capture(&nothing, &camera, false, "9d-xray-cube-empty")
    };

    let alphas: Vec<f64> = ring_pixels(&over_form, &bare)
        .into_iter()
        .filter(|at| on_form(&bare, ground, *at))
        .filter(|at| light(&whole, *at).abs_diff(light(&empty, *at)) > 24)
        .filter_map(|at| drawn_alpha(&over_form, &bare, &whole, at))
        .collect();
    assert!(
        alphas.len() > 30,
        "the gizmo's alpha could be measured at only {} pixels over the form, \
         so this measures nothing — see target/visual/9d-xray-cube.png",
        alphas.len()
    );
    let faint = alphas.iter().filter(|a| **a < 0.6).count();
    println!(
        "the orientation gizmo over the sculpt was drawn at mean alpha {:.2} \
         over {} pixels",
        mean(&alphas),
        alphas.len()
    );
    assert_eq!(
        faint,
        0,
        "{faint} of {} pixels of the orientation gizmo were dimmed by a sculpt \
         that says nothing about where the gizmo is — see \
         target/visual/9d-xray-cube.png",
        alphas.len()
    );
}

/// A ghosted surface dims nothing, because it writes no depth.
///
/// Not a special case in the dimming, and that is why it is worth asserting:
/// the ghost pipelines were already unculled and depth-writeless, which is
/// "what lets the far half of the cage read through the form" in their own
/// words. So a cage being edited keeps the scaffolding it has always had,
/// through the depth buffer being empty rather than through an exception that
/// could rot.
#[test]
fn a_ghosted_surface_dims_nothing() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };
    let ground = harness.background();
    let nothing = clayspace_view::GpuMesh::new(&harness.gpu);

    harness
        .renderer
        .set_surface_opacity(clayspace_model::SurfaceOpacity::CAGED);
    no_ring(&mut harness);
    let bare = harness.capture(geometry.mesh(), &camera, false, "9d-xray-ghost-bare");
    let empty = harness.capture(&nothing, &camera, false, "9d-xray-ghost-empty");
    ring_around_the_form(&mut harness);
    let ringed = harness.capture(geometry.mesh(), &camera, false, "9d-xray-ghost-ring");
    let whole = harness.capture(&nothing, &camera, false, "9d-xray-ghost-whole");

    let alphas = alphas_over_the_form(&ringed, &bare, &whole, &empty, ground);
    assert!(
        alphas.len() > 300,
        "the ring's alpha could be measured at only {} pixels over the ghosted \
         form, so this measures nothing",
        alphas.len()
    );
    let faint = alphas.iter().filter(|a| **a < 0.6).count();
    let share = faint as f64 / alphas.len() as f64;
    println!(
        "over a ghosted form the ring was drawn at mean alpha {:.2}",
        mean(&alphas)
    );
    assert!(
        share < 0.1,
        "{:.1}% of the ring crossing a ghosted form was dimmed, and a ghost \
         writes no depth to be in front of anything — see \
         target/visual/9d-xray-ghost-ring.png",
        share * 100.0
    );
}
