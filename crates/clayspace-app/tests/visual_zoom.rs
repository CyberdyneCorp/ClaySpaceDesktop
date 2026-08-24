//! Zooming, against the thing being zoomed at.
//!
//! It went inside the model. The camera's distance was clamped to an arbitrary
//! floor rather than to the clay, so a few notches too many put the eye through
//! the surface and the sculpt turned inside out.
//!
//! Blender's answer is two things at once, and this is both: the zoom stops a
//! little short of what is under the pointer, and the pivot follows part of the
//! way so the next orbit turns around what was being looked at.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_zoom
//! open target/visual
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::SculptModel;
use clayspace_view::{Camera, Image};
use support::Harness;

fn sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// How much of the frame the form covers.
///
/// Going *through* a surface reads as the form filling the frame and then
/// vanishing — inside a closed shape there is nothing facing the camera — so
/// this is the measurement that catches it.
///
/// Against the renderer's own background rather than the corner pixel. A
/// corner is a fine stand-in for the background until the form fills the
/// frame, which is exactly the case this is watching: at that point the corner
/// *is* clay and everything reads as background, so a full frame measures as
/// an empty one.
fn covered(image: &Image, ground: [u8; 4]) -> f64 {
    let lit = image
        .pixels
        .chunks_exact(4)
        .filter(|p| (0..3).any(|c| p[c].abs_diff(ground[c]) > 10))
        .count();
    lit as f64 / (image.width * image.height) as f64
}

#[test]
fn zooming_in_never_ends_up_inside_the_form() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let ground = harness.background();
    let Some(mut document) = sphere() else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.sync(&harness.gpu, &mut document).expect("mesh");

    let mut camera = Camera::default();
    camera.frame_bounds([-1.0, -1.0, -1.0].into(), [1.0, 1.0, 1.0].into());

    // Straight at the middle of the form, which is where the pointer would be.
    let focus = |camera: &Camera| -> Option<[f32; 3]> {
        let eye = camera.eye();
        let toward = (camera.target - eye).normalize();
        SculptModel::pick(&document, eye.into(), toward.into())
    };

    let mut shots = Vec::new();
    for notch in 0..40 {
        camera.zoom_toward(1.0, focus(&camera));
        if notch % 10 == 9 {
            shots.push(harness.capture(
                geometry.mesh(),
                &camera,
                false,
                &format!("zoom-{}", notch + 1),
            ));
        }
    }

    // Still looking at clay. Inside a closed form there is nothing facing the
    // camera, so the frame would go empty.
    for (at, shot) in shots.iter().enumerate() {
        let filled = covered(shot, ground);
        assert!(
            filled > 0.2,
            "after {} notches the form covers {filled:.3} of the frame, which \
             is the camera having gone through it. See target/visual/zoom-*.png",
            (at + 1) * 10
        );
    }

    // And it did come closer: the form fills more of the frame than it did.
    assert!(
        covered(shots.last().expect("a shot"), ground) > covered(&shots[0], ground),
        "forty notches of zoom did not bring the form any closer"
    );
}

#[test]
fn the_old_behaviour_would_have_failed_this() {
    // Worth stating, because a test that passes on both the fix and the fault
    // is not a test. The plain multiplicative zoom drives the distance to its
    // arbitrary floor, which is well inside a unit sphere.
    let mut camera = Camera::default();
    camera.frame_bounds([-1.0, -1.0, -1.0].into(), [1.0, 1.0, 1.0].into());
    for _ in 0..100 {
        camera.zoom(1.0);
    }
    assert!(
        camera.distance < 1.0,
        "the plain zoom stopped at {} from the pivot, so it never went inside \
         the form and this file is testing nothing",
        camera.distance
    );
}
