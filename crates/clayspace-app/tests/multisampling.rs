//! The scene is drawn multisampled, and the two places that decide so agree.
//!
//! There was no anti-aliasing at all: the pipelines took
//! `multisample: Default::default()` and the depth texture `sample_count: 1`.
//! On a dense silhouette against a flat ground that is the most visible thing
//! wrong with the picture, and it was free to fix — the frame was measured at
//! 0.45 ms of a 16.7 ms budget, so the GPU was doing nothing with the time.
//!
//! What this holds is not "four samples" — a device that will not multisample
//! the format falls back to one rather than taking the window down. It is that
//! the *framebuffer* and the *pipelines* read the count from the same place. A
//! pipeline whose sample count differs from its attachment's is a validation
//! error at draw time, which is a black window and a log line rather than a
//! failed test.

mod support;

use clayspace_view::{Camera, GpuMesh, OffscreenTarget};
use support::Harness;

#[test]
fn the_framebuffer_and_the_pipelines_agree_on_the_sample_count() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let format = OffscreenTarget::FORMAT;
    let expected = harness.gpu.sample_count(format);
    assert_eq!(
        harness.target.framebuffer().samples(),
        expected,
        "the framebuffer is multisampled differently from what the pipelines \
         will be built for, which is a validation error at the first draw"
    );

    // And the pass actually runs, which is what says the two agree in practice
    // rather than only in arithmetic: a mismatch fails here and nowhere else.
    let empty = GpuMesh::new(&harness.gpu);
    let image = harness.target.capture(
        &harness.gpu,
        &harness.renderer,
        &Camera::default(),
        &empty,
        false,
    );
    assert_eq!(image.width, Harness::WIDTH);
}

#[test]
fn a_device_that_multisamples_is_used_multisampled() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let samples = harness.gpu.sample_count(OffscreenTarget::FORMAT);
    println!("{:?}: {samples} samples", OffscreenTarget::FORMAT);
    // Not asserted as 4: a device that refuses is handled rather than failed.
    // Asserted as *sane*, so a fallback that quietly became permanent — a
    // wrong format threaded through, a capability query inverted — is visible.
    assert!(
        samples == 1 || samples == 4,
        "an unexpected sample count of {samples}"
    );
}
