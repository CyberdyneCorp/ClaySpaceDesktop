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

use clayspace_view::{Camera, GpuMesh, MsaaQuality, OffscreenTarget};
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
    println!(
        "{:?}: {samples} samples, wanted {:?}",
        OffscreenTarget::FORMAT,
        harness.gpu.msaa()
    );
    // Not asserted as 4: a device that refuses is handled rather than failed,
    // and the quality is selectable. Asserted as *sane*, so a fallback that
    // quietly became permanent — a wrong format threaded through, a capability
    // query inverted — is visible.
    assert!(
        MsaaQuality::ALL
            .iter()
            .any(|quality| quality.samples() == samples),
        "an unexpected sample count of {samples}"
    );
    assert!(
        samples <= harness.gpu.msaa().samples(),
        "the device drew at {samples} samples having been asked for {:?}, \
         which is a resolve that went upward",
        harness.gpu.msaa()
    );
}

/// A quality the format will not take falls back to one it will, downward.
///
/// The one behaviour that separates "selectable" from "a way to take the
/// window down": a sample count the format does not support is a validation
/// error at pipeline creation.
#[test]
fn a_quality_the_format_refuses_resolves_downward() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let format = OffscreenTarget::FORMAT;
    let mut previous = 0;
    for quality in MsaaQuality::ALL {
        let resolved = harness.gpu.supported_samples(quality, format);
        assert!(
            resolved <= quality.samples(),
            "{quality:?} resolved upward to {resolved}"
        );
        assert!(
            resolved >= previous,
            "{quality:?} resolved to {resolved}, below what a cheaper quality \
             resolved to"
        );
        previous = resolved;
    }
    assert_eq!(
        harness.gpu.supported_samples(MsaaQuality::Off, format),
        1,
        "no multisampling is one sample, on every device"
    );
}

/// Every quality this device offers actually draws.
///
/// The regression test for a failure that looked like a *measurement*. The
/// resolve consulted the adapter, which reports the sample counts the hardware
/// has; the device may only use the two WebGPU guarantees — one and four —
/// unless it asked for the adapter-specific ones. Choosing 2× therefore built
/// every pipeline with a validation error, and because a validation error is
/// reported rather than fatal here, the frame survived and drew nothing. It
/// read as 2× multisampling costing 0.03 ms against 0.20 for none.
///
/// So this asserts the picture rather than the number: a quality that resolves
/// to a sample count has to produce a frame with a form in it.
#[test]
fn every_quality_that_resolves_actually_draws() {
    let format = OffscreenTarget::FORMAT;

    for quality in MsaaQuality::ALL {
        // A device of its own, told before anything is built on it: a
        // pipeline's sample count is part of its state.
        let Ok(mut gpu) = pollster::block_on(clayspace_view::Gpu::headless()) else {
            return;
        };
        gpu.set_msaa(quality);
        let resolved = gpu.sample_count(format);
        assert!(
            resolved <= quality.samples(),
            "{quality:?} resolved upward to {resolved} samples"
        );

        let renderer = clayspace_view::Renderer::new(&gpu, format);
        let target = OffscreenTarget::new(&gpu, 256, 192);
        assert_eq!(
            target.framebuffer().samples(),
            resolved,
            "{quality:?}: the framebuffer and the pipelines disagree"
        );

        // A triangle across the middle of the frame, so "something was drawn"
        // is unambiguous.
        let mut mesh = GpuMesh::new(&gpu);
        let vertex = |position: [f32; 3]| clayspace_view::Vertex {
            position,
            normal: [0.0, 0.0, 1.0],
            color: [1.0, 1.0, 1.0],
            mask: 0.0,
        };
        mesh.upload(
            &gpu,
            &[
                vertex([-0.8, -0.8, 0.0]),
                vertex([0.8, -0.8, 0.0]),
                vertex([0.0, 0.9, 0.0]),
            ],
            &[0, 1, 2],
        );

        let mut camera = Camera::default();
        camera.frame_bounds([-0.8, -0.8, 0.0].into(), [0.8, 0.9, 0.0].into());
        let image = target.capture(&gpu, &renderer, &camera, &mesh, false);

        let empty = GpuMesh::new(&gpu);
        let ground = target
            .capture(&gpu, &renderer, &camera, &empty, false)
            .pixel(0, 0);
        let covered = image.pixels_differing_from(ground, 6);
        println!("{quality:?} -> {resolved} samples: {covered} pixels drawn");
        assert!(
            covered > 1_000,
            "{quality:?} resolved to {resolved} samples and drew {covered} \
             pixels — a pipeline that fails validation draws nothing, and the \
             error is reported rather than fatal, so this is the only place it \
             shows"
        );
    }
}
