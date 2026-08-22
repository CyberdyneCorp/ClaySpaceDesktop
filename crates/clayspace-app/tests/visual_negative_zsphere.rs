//! What a negative ZSphere looks like, before and after ClayCore 0.30.0's
//! signs (#99).
//!
//! `armature_signs.rs` probes the surface at points and says whether it is
//! solid. That catches the membrane being drawn through a cut, and says
//! nothing about whether the result reads as an indentation. This renders it.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_negative_zsphere
//! open target/visual
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{ArmatureModel, SculptModel};
use clayspace_view::Camera;
use support::Harness;

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy).ok()
}

fn framed(document: &ClayDocument) -> Camera {
    let mut camera = Camera::default();
    match SculptModel::bounds(document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }
    camera
}

#[test]
fn a_negative_sphere_reads_as_an_indentation() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };

    // A head with a socket pushed into it, which is the shape the feature is
    // for: a big sphere, a small one overlapping its surface.
    document.begin_armature([0.0, 0.0, 0.0], 0.6).expect("head");
    let socket = document
        .add_zsphere(0, [0.38, 0.15, 0.38], 0.22, false)
        .expect("socket");

    let camera = framed(&document);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the rig as authored");
    harness.capture(geometry.mesh(), &camera, false, "19-zsphere-positive");

    document
        .set_zsphere_negative(socket, true)
        .expect("a negative sphere");
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the rig with the socket cutting");
    harness.capture(geometry.mesh(), &camera, false, "19-zsphere-negative");

    // Turning it back is the same picture again, so the sign is a toggle
    // rather than a demolition.
    document
        .set_zsphere_negative(socket, false)
        .expect("positive again");
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the rig restored");
    harness.capture(geometry.mesh(), &camera, false, "19-zsphere-restored");
}
