//! The tendril a snakehook pulls, looked at.
//!
//! It came out a string of beads. A drag arrives in segments, and each segment
//! authored its *own* curve item — restarting the taper from full width every
//! time — so a curving pull left a chain of spheres rather than a tendril.
//! Measured on one such pull, the thickness along it wobbled by 0.210 where a
//! single curve wobbles by 0.137, and that 0.137 is the taper itself.
//!
//! Two things were wrong and both are fixed here: the gesture grows one curve
//! now, and its points are joined by a Catmull-Rom spline rather than the
//! straight spans a stroke's points take by default.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_snakehook
//! open target/visual
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use clayspace_view::Camera;
use support::Harness;

fn sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// A curving pull, which is what a sculptor makes and where a chain of hard
/// corners shows. A straight one hides the fault entirely.
fn curving() -> Vec<GestureSample> {
    (0..=16)
        .map(|step| {
            let t = step as f32 / 16.0;
            let a = t * 1.8;
            GestureSample {
                position: [a.sin() * 1.0, 0.0, 0.95 + a.cos() * 0.7 - 0.7],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.18,
        intensity: 0.9,
        ..BrushSettings::default()
    }
}

/// Pulls the tendril the way the interface delivers a drag: a gesture, and
/// segments each carrying the whole path so far.
fn pull(document: &mut ClayDocument) {
    let path = curving();
    document.begin_gesture();
    for end in [3usize, 6, 9, 12, 16] {
        document
            .apply_stroke(
                ToolKind::Puxar,
                brush(),
                &path[..=end.min(path.len() - 1)],
                [false; 3],
            )
            .expect("the pull was refused");
    }
    document.end_gesture();
}

/// How thick the tendril is at each point of the path, and how much that
/// wobbles from one to the next.
///
/// The wobble is the measurement that matters: a taper is a *monotone* change
/// in thickness, and beading is the same change with oscillation on top.
///
/// The arc is kept short enough that the tendril does not curve back over the
/// form it came from. Past about two radians the measuring ray at the tip
/// meets the sphere rather than the tendril and the numbers jump — a fault in
/// the probe, not in the pull.
fn profile(document: &ClayDocument) -> (Vec<f32>, f32) {
    let widths: Vec<f32> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.0;
            let a = t * 1.8;
            let at = [a.sin() * 1.0, 0.0, 0.95 + a.cos() * 0.7 - 0.7];
            SculptModel::pick(document, [at[0], 3.0, at[2]], [0.0, -1.0, 0.0])
                .map(|hit| hit[1])
                .unwrap_or(f32::NAN)
        })
        .collect();
    let wobble = widths
        .windows(2)
        .filter(|pair| pair[0].is_finite() && pair[1].is_finite())
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum();
    (widths, wobble)
}

#[test]
fn a_pulled_tendril_tapers_rather_than_beads() {
    let Some(mut document) = sphere() else {
        return;
    };
    pull(&mut document);
    let (widths, wobble) = profile(&document);

    assert!(
        widths.iter().all(|w| w.is_finite()),
        "the pull left a gap in the tendril: {widths:?}"
    );
    assert!(
        wobble < 0.16,
        "the thickness along the tendril wobbled by {wobble:.3}, where a \
         string of beads measures 0.210 and a single tapering curve 0.137. \
         {widths:?}"
    );

    // A taper: thinner at the tip than at the root. Not monotone throughout —
    // the tendril swells a little where it leaves the sphere it was pulled
    // from — so this is the two ends rather than every step.
    let root = widths[2];
    let tip = widths[widths.len() - 1];
    assert!(
        tip < root - 0.02,
        "the tendril is {tip} thick at the tip and {root} at the root, which \
         is a tube rather than something pulled"
    );
}

#[test]
fn one_gesture_pulls_one_tendril() {
    // The fault itself: a segment that authored its own item left a *trail* of
    // curves, each restarting the taper. One gesture is one curve, and one
    // undo takes it.
    let Some(mut document) = sphere() else {
        return;
    };
    let before = document.history().depth;
    pull(&mut document);
    let after = document.history().depth;

    assert!(after > before, "the pull recorded nothing at all");
    let reach = SculptModel::pick(&document, [3.0, 0.0, 0.35], [-1.0, 0.0, 0.0])
        .map(|hit| hit[0])
        .unwrap_or(0.0);
    assert!(reach > 1.0, "the tendril did not reach out from the form");

    // Undone entirely, however many segments drew it.
    for _ in before..after {
        document.undo().expect("undo");
    }
    let left = SculptModel::pick(&document, [3.0, 0.0, 0.35], [-1.0, 0.0, 0.0])
        .map(|hit| hit[0])
        .unwrap_or(0.0);
    assert!(
        left < 1.02,
        "undoing the gesture left {left} of the tendril behind"
    );
}

#[test]
fn a_second_pull_is_its_own_tendril() {
    // The curve is held only while a gesture is open. Held past it, the next
    // pull would go on growing the first.
    let Some(mut document) = sphere() else {
        return;
    };
    pull(&mut document);
    let first = SculptModel::pick(&document, [3.0, 0.0, 0.35], [-1.0, 0.0, 0.0])
        .map(|hit| hit[0])
        .unwrap_or(0.0);

    // A pull the other way.
    let path: Vec<GestureSample> = (0..=12)
        .map(|step| {
            let t = step as f32 / 12.0;
            GestureSample {
                position: [0.0, 0.95 + t * 0.7, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document.begin_gesture();
    for end in [3usize, 8, 12] {
        document
            .apply_stroke(ToolKind::Puxar, brush(), &path[..=end], [false; 3])
            .expect("the pull was refused");
    }
    document.end_gesture();

    let up = SculptModel::pick(&document, [0.0, 3.0, 0.0], [0.0, -1.0, 0.0])
        .map(|hit| hit[1])
        .unwrap_or(0.0);
    assert!(up > 1.4, "the second pull did not reach up: {up}");
    let still = SculptModel::pick(&document, [3.0, 0.0, 0.35], [-1.0, 0.0, 0.0])
        .map(|hit| hit[0])
        .unwrap_or(0.0);
    assert!(
        (still - first).abs() < 0.02,
        "the second pull moved the first tendril, from {first} to {still} — \
         the curve outlived its gesture and the next drag grew it"
    );
}

#[test]
fn the_tendril_is_drawn_as_one_form() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = sphere() else {
        return;
    };
    let mut camera = Camera::default();
    camera.frame_bounds([-1.7, -1.7, -1.7].into(), [1.7, 1.7, 1.7].into());
    // From above. The tendril curves in the x-z plane, so a camera looking
    // along it sees the pull end-on and the sphere hides the rest.
    camera.pitch = -1.25;

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("the first mesh");
    let before = harness.capture(geometry.mesh(), &camera, false, "snakehook-before");

    pull(&mut document);
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("the re-mesh");
    let after = harness.capture(geometry.mesh(), &camera, false, "snakehook-after");

    let changed = before
        .pixels
        .chunks_exact(4)
        .zip(after.pixels.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 12))
        .count();
    assert!(
        changed > 1500,
        "the pull changed {changed} pixels. See \
         target/visual/snakehook-after.png"
    );
}
