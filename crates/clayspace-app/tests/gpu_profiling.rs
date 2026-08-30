//! Per-pass GPU timing, and the two ways it took the device down with it.
//!
//! The profiler is diagnostics: it must never be able to affect whether a frame
//! is drawn. Both bugs it shipped with broke exactly that rule, and neither was
//! visible from a test that captured a frame — a capture reads the target back
//! and waits for the device on every frame, which hides both.
//!
//! **A query resolved but never written blocks the device.** The first version
//! kept one query set with a pair of slots per pass and resolved the whole set.
//! Resolving a query that was never written is a wait on a result that will
//! never become available: the frame never completes, and sixty seconds later
//! the driver gives up. Any frame with occlusion switched off did it — which is
//! every frame of every capture that compares occlusion on against off.
//!
//! **A readback mapped twice is a panic.** The second version asked for the map
//! once a frame for as long as the result was in flight, rather than once per
//! resolve. A frame that does not wait for the device leaves the result in
//! flight, so the next frame maps an already-mapped buffer, and wgpu asserts.
//!
//! So this renders frames the way a window does — submit, poll without waiting,
//! carry on — rather than the way a capture does.

mod support;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_view::{Camera, GpuPass};
use support::Harness;

/// Renders `frames` frames without ever waiting for the device.
///
/// `Maintain::Poll` rather than `Maintain::Wait`: the point is to leave work in
/// flight across frame boundaries, which is what a presenting window does and
/// what an offscreen capture never does.
fn render_without_waiting(harness: &Harness, mesh: &clayspace_view::GpuMesh, frames: usize) {
    let camera = Camera::default();
    for _ in 0..frames {
        harness.renderer.render(
            &harness.gpu,
            harness.target.view(),
            harness.target.framebuffer(),
            &camera,
            mesh,
            false,
        );
        let _ = harness.gpu.device.poll(wgpu::Maintain::Poll);
    }
}

fn worked(harness: &Harness) -> Option<SurfaceGeometry> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = Scene::Reference.build(policy).ok()?;
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, &mut document).ok()?;
    Some(geometry)
}

/// Frames that do not wait for the device are drawn, and go on being drawn.
#[test]
fn frames_that_never_wait_for_the_device_still_draw() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(geometry) = worked(&harness) else {
        return;
    };

    // Sixty of them: more than enough for a result to still be in flight when
    // the next frame is encoded, which is the condition that panicked.
    render_without_waiting(&harness, geometry.mesh(), 60);

    // And the frame after them is a real one. The device has to still be
    // answering for this to come back at all.
    let camera = Camera::default();
    let image = harness.capture(geometry.mesh(), &camera, false, "99-profiled-frame");
    let covered = image.pixels_differing_from(harness.background(), 6);
    assert!(
        covered > 1_000,
        "only {covered} pixels were drawn after sixty unwaited frames"
    );
}

/// A frame with occlusion switched off leaves three of the four passes without
/// timestamps, and must still complete.
///
/// This is the one that hung. Every occlusion capture in the suite renders a
/// frame with the passes off and a frame with them on; the first of those has
/// three unwritten query pairs in it.
#[test]
fn a_frame_that_skips_passes_still_completes() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(geometry) = worked(&harness) else {
        return;
    };
    let camera = Camera::default();

    for occlusion in [false, true, false, true] {
        harness.renderer.set_occlusion(occlusion);
        // Waited rather than polled, so a device that has stopped answering
        // stops this test rather than the one above.
        let image = harness.capture(
            geometry.mesh(),
            &camera,
            false,
            if occlusion {
                "99-profiled-occluded"
            } else {
                "99-profiled-plain"
            },
        );
        let covered = image.pixels_differing_from(harness.background(), 6);
        assert!(
            covered > 1_000,
            "a frame with occlusion {} drew {covered} pixels",
            if occlusion { "on" } else { "off" }
        );
    }
}

/// What is reported is what ran.
///
/// The scene pass runs in every frame; the three occlusion passes run only when
/// occlusion is on. A profiler that reported a pass that did not run would be
/// reporting a number from whichever frame last wrote that slot.
#[test]
fn only_the_passes_that_ran_are_reported() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    if !harness.renderer.gpu_timing_available() {
        eprintln!("skipping: this adapter reports no GPU timestamps");
        return;
    }
    let Some(geometry) = worked(&harness) else {
        return;
    };
    let camera = Camera::default();

    // Several frames, because the read is deliberately one frame behind and
    // the first has nothing to report.
    harness.renderer.set_occlusion(true);
    for _ in 0..8 {
        let _ = harness.target.capture(
            &harness.gpu,
            &harness.renderer,
            &camera,
            geometry.mesh(),
            false,
        );
    }
    let with = harness.renderer.gpu_timing().expect("a measured frame");
    let measured: Vec<GpuPass> = with.measured().map(|(pass, _)| pass).collect();
    println!("occlusion on: {measured:?}");
    assert!(
        measured.contains(&GpuPass::Scene),
        "the scene pass was not measured"
    );
    assert!(
        measured.iter().any(|pass| *pass != GpuPass::Scene),
        "no occlusion pass was measured with occlusion on"
    );
    assert!(with.total() > 0.0, "every pass reported zero time");

    harness.renderer.set_occlusion(false);
    for _ in 0..8 {
        let _ = harness.target.capture(
            &harness.gpu,
            &harness.renderer,
            &camera,
            geometry.mesh(),
            false,
        );
    }
    let without = harness.renderer.gpu_timing().expect("a measured frame");
    let measured: Vec<GpuPass> = without.measured().map(|(pass, _)| pass).collect();
    println!("occlusion off: {measured:?}");

    // The claim is the converse of the one above, and it is the one that
    // matters: a pass that did not run must not be reported. Reported as a
    // subset rather than as an exact list, because a pass whose start and end
    // timestamps come back equal is dropped rather than reported as zero — a
    // device is entitled to answer that way for a pass that costs almost
    // nothing, and it is not what this test is about.
    assert_eq!(
        measured,
        vec![GpuPass::Scene],
        "an occlusion pass was reported for a frame that ran none"
    );
    assert!(
        without.get(GpuPass::Ao).is_none(),
        "a pass that did not run reported a time"
    );
}
