//! Occlusion darkens the sculpt, and stops there.
//!
//! The occlusion composite multiplies over everything already drawn, so *when*
//! a thing is drawn decides whether it is shaded by the form behind it. For a
//! helper lying on the clay — the cursor ring, the polyframe, the rig — being
//! shaded with the clay is right, and they are drawn before it. For the
//! scaffolding it is not: a manipulator stands *over* the form rather than on
//! it, and the handle a person is aiming at came out dimmed exactly where the
//! form is deepest, which is where they are most likely to be aiming.
//!
//! So the scaffolding is drawn after the composite, and this holds it there.
//!
//! The scaffolding *is* drawn faint where the sculpt stands in front of it,
//! which is a depth cue and not occlusion: it comes from one comparison against
//! the depth the sculpt wrote, it is the same with occlusion on and off, and
//! the reduction it reads runs either way for exactly that reason. What it does
//! change is what a *faint* pixel contains — forty percent widget over sixty
//! percent form — so a faint pixel darkens with occlusion because the form
//! showing through it does. The invariant here is therefore stated over the
//! pixels the manipulator covers opaquely, which is where the question "was the
//! widget darkened" has an answer at all.

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

/// A manipulator over the middle of the form.
fn manipulator_over_the_form(harness: &mut Harness) {
    let gpu = harness.gpu.clone();
    harness.renderer.set_lattice(
        &gpu,
        LatticeView {
            points: &[],
            edges: &[],
            selected: &[],
            gizmo: Some(GizmoView {
                pivot: [0.0, 0.0, 0.0],
                mode: GizmoMode::Move,
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

/// No manipulator, for the frames that say where it was.
fn no_manipulator(harness: &mut Harness) {
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

/// Which pixels the manipulator covers, as the difference between a frame with
/// it and a frame without.
fn scaffolding_pixels(with: &Image, without: &Image) -> Vec<(u32, u32)> {
    (0..with.height)
        .flat_map(|y| (0..with.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let (a, b) = (with.pixel(*x, *y), without.pixel(*x, *y));
            (0..3).any(|c| a[c].abs_diff(b[c]) > 24)
        })
        .collect()
}

/// How much darker occlusion has to make a pixel before it counts.
///
/// Well above what two draws of the same geometry differ by — the support
/// module measures that at a level or two, on the frames that were meant to be
/// unchanged — and well below what the thing being ruled out would do:
/// occlusion takes up to a sixth of a pixel's light, which on a manipulator
/// drawn near white is some thirty levels. A factor of four either side.
const DARKENED: i32 = 8;

/// The manipulator is drawn at the colours it was given, whatever the
/// occlusion under it does.
#[test]
fn occlusion_does_not_darken_the_manipulator() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };

    // Four frames: with the manipulator and without, under occlusion and
    // without it. The pair without the manipulator is what says where the
    // manipulator *is*, and it is taken under both occlusion states rather
    // than one, because two draws of the same geometry do not agree to the
    // pixel on every device — a line that lands exactly on a pixel's edge can
    // be rasterized into it in one frame and not the other. Judging only the
    // pixels the manipulator covers in *both* leaves that disagreement out by
    // construction, rather than tolerating some number of it.
    harness.renderer.set_occlusion(true);
    let bare_occluded = harness.capture(geometry.mesh(), &camera, false, "9a-order-bare");
    manipulator_over_the_form(&mut harness);
    let occluded = harness.capture(geometry.mesh(), &camera, false, "9a-order-occluded");

    harness.renderer.set_occlusion(false);
    let plain = harness.capture(geometry.mesh(), &camera, false, "9a-order-plain");
    // The same manipulator with nothing behind it, which is its own colour at
    // full strength: the scaffolding is drawn faint where the sculpt stands in
    // front of it, and nothing stands in front of it here.
    let nothing = clayspace_view::GpuMesh::new(&harness.gpu);
    let whole = harness.capture(&nothing, &camera, false, "9a-order-whole");
    no_manipulator(&mut harness);
    let bare_plain = harness.capture(geometry.mesh(), &camera, false, "9a-order-bare-plain");

    let under_occlusion: std::collections::HashSet<(u32, u32)> =
        scaffolding_pixels(&occluded, &bare_occluded)
            .into_iter()
            .collect();
    let covered: Vec<(u32, u32)> = scaffolding_pixels(&plain, &bare_plain)
        .into_iter()
        .filter(|at| under_occlusion.contains(at))
        // Only where the manipulator is drawn *opaquely*, which is where it is
        // in front of the form or clear of it.
        //
        // Where it is drawn faint the form shows through it, and that form is
        // legitimately shaded by occlusion — so such a pixel does darken, and
        // what darkened is the sculpt behind the widget rather than the widget.
        // Opaque is read off the frame rather than assumed: a pixel that is the
        // manipulator's own colour with the form there and with it removed had
        // nothing of the form in it.
        .filter(|(x, y)| {
            let (drawn, full) = (plain.pixel(*x, *y), whole.pixel(*x, *y));
            (0..3).all(|c| drawn[c].abs_diff(full[c]) <= 8)
        })
        .collect();
    assert!(
        covered.len() > 300,
        "the manipulator was drawn opaquely over only {} pixels of both \
         frames, so this measures nothing — see \
         target/visual/9a-order-occluded.png",
        covered.len()
    );

    // What the invariant says is that occlusion reached the form and *not* the
    // manipulator, so that is what is measured: how often each darkens.
    //
    // Not "not one pixel of the manipulator moved". That was the first
    // formulation and it failed twice on macOS, on a single pixel out of eight
    // thousand, by fifteen levels — and the second attempt, which judged only
    // the pixels the manipulator covers in *both* frames, failed the same way.
    // Two renders of the same geometry are not bit-identical on every device,
    // which this suite's own noise floor exists to say; chasing the last pixel
    // was tuning a threshold rather than testing a property.
    //
    // The relative form has no threshold to tune and far more power. If the
    // composite reached the manipulator it would darken it about as often as
    // it darkens the form — the two would be within a factor of one of each
    // other, not a hundred.
    let darkening_at = |x: u32, y: u32| {
        let (a, b) = (plain.pixel(x, y), occluded.pixel(x, y));
        (0..3).map(|c| a[c] as i32 - b[c] as i32).max().unwrap_or(0)
    };
    let fraction_over = |pixels: &[(u32, u32)]| {
        let over = pixels
            .iter()
            .filter(|(x, y)| darkening_at(*x, *y) > DARKENED)
            .count();
        (over, over as f64 / pixels.len().max(1) as f64)
    };

    let ground = harness.background();
    let manipulator: std::collections::HashSet<(u32, u32)> = covered.iter().copied().collect();
    let form: Vec<(u32, u32)> = (0..plain.height)
        .flat_map(|y| (0..plain.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let p = bare_plain.pixel(*x, *y);
            !manipulator.contains(&(*x, *y)) && (0..3).any(|c| p[c].abs_diff(ground[c]) >= 6)
        })
        .collect();

    let (on_manipulator, manipulator_share) = fraction_over(&covered);
    let (on_form, form_share) = fraction_over(&form);
    println!(
        "occlusion darkened {on_manipulator} of {} manipulator pixels ({:.3}%) and \
         {on_form} of {} form pixels ({:.3}%)",
        covered.len(),
        manipulator_share * 100.0,
        form.len(),
        form_share * 100.0
    );

    assert!(
        form_share > 0.02,
        "occlusion darkened {:.3}% of the form, so this measures nothing",
        form_share * 100.0
    );
    assert!(
        manipulator_share * 100.0 < form_share,
        "occlusion darkened {:.3}% of the manipulator against {:.3}% of the \
         form behind it — a hundredfold apart is what drawing after the \
         composite looks like, and this is not",
        manipulator_share * 100.0,
        form_share * 100.0
    );
}

/// And the sculpt still is. A pass order that stopped occlusion reaching the
/// scaffolding by stopping it reaching anything would satisfy the test above.
#[test]
fn occlusion_still_darkens_the_sculpt_beneath_it() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, camera)) = worked(&harness) else {
        return;
    };
    manipulator_over_the_form(&mut harness);

    harness.renderer.set_occlusion(false);
    let without = harness.capture(geometry.mesh(), &camera, false, "9a-order-surface-off");
    harness.renderer.set_occlusion(true);
    let with = harness.capture(geometry.mesh(), &camera, false, "9a-order-surface-on");

    let darker = (0..with.height)
        .flat_map(|y| (0..with.width).map(move |x| (x, y)))
        .filter(|(x, y)| without.pixel(*x, *y)[0] as i32 - with.pixel(*x, *y)[0] as i32 > 8)
        .count();
    println!("the form under the manipulator: {darker} pixels darkened");
    assert!(
        darker > 100,
        "only {darker} pixels darkened, so occlusion is not reaching the sculpt"
    );
}

/// The orientation gizmo is not occludable by a sculpt that reaches into its
/// corner.
///
/// It was, and silently: it is drawn in a corner viewport with its own camera,
/// but it was depth-tested against the *scene's* depth buffer, whose contents
/// in those pixels are whatever the sculpt happened to write there.
#[test]
fn the_orientation_gizmo_is_not_hidden_by_the_sculpt() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((geometry, _)) = worked(&harness) else {
        return;
    };
    // Close enough that the form covers the corner the gizmo sits in.
    let mut camera = Camera {
        distance: 1.6,
        ..Camera::default()
    };
    camera.orbit(0.3, -0.2);

    harness.renderer.show_gizmo = false;
    let without = harness.capture(geometry.mesh(), &camera, false, "9a-order-gizmo-off");
    harness.renderer.show_gizmo = true;
    let with = harness.capture(geometry.mesh(), &camera, false, "9a-order-gizmo-on");
    harness.renderer.show_gizmo = false;

    // The gizmo sits in the top-right quarter of the frame.
    let drawn = (0..with.height / 2)
        .flat_map(|y| (with.width / 2..with.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let (a, b) = (with.pixel(*x, *y), without.pixel(*x, *y));
            (0..3).any(|c| a[c].abs_diff(b[c]) > 24)
        })
        .count();
    println!("orientation gizmo over the sculpt: {drawn} pixels drawn");
    // A hundred, against the 192 measured: the gizmo is six short lines and a
    // few labels, so how many pixels it lands on depends on how a device
    // rasterizes a line, and the claim here is "it is there" rather than "it
    // is this many". Zero is what a depth-tested gizmo behind the sculpt drew.
    assert!(
        drawn > 100,
        "the gizmo drew {drawn} pixels over a sculpt filling its corner — see \
         target/visual/9a-order-gizmo-on.png"
    );
}
