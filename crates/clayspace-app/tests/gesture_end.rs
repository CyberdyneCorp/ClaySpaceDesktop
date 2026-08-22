//! What the end of a gesture costs, which is what a sculptor feels as a hitch.
//!
//! Lifting the pointer used to run a second shading pass over every key the
//! stroke had touched — 111 of them on the gesture below — because sculpting
//! shaded with face normals and bought the gradient back afterwards. The
//! application reported it, in Portuguese, on every stroke:
//!
//! ```text
//! a interface travou: sombreamento final 17 ms
//! a interface travou: stroke 20 ms
//! ```
//!
//! Those two are one event: `stroke` is the outer timer around `EndStroke` and
//! the shading pass ran inside it. The split existed because the gradient once
//! cost eleven times face normals; by ClayCore 0.30.0 it cost 1.04x, so the
//! pass was spending 15.7 ms buying back something worth 0.6 ms. Sculpting
//! shades fully now and there is no second pass.
//!
//! What this holds is the end of a gesture staying inside a frame. That the
//! drag itself still draws what a full re-mesh would is `visual_incremental`'s
//! job, and its mid-drag comparison got *tighter* with this change rather than
//! looser — mid-drag is now the same shading a rebuild uses.

mod support;

use std::time::Instant;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind, FRAME};
use support::Harness;

/// Segments in the test gesture. Enough that the old refinement pass had a
/// gesture's worth of keys to re-mesh rather than a dab's.
const SEGMENTS: usize = 24;

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// Drags across the front of the model, syncing after each segment as the
/// application does while the pointer is down.
///
/// Returns the worst segment and the whole gesture's cost.
fn drag(
    harness: &Harness,
    geometry: &mut SurfaceGeometry,
    document: &mut ClayDocument,
) -> (f64, f64) {
    let mut worst = 0.0f64;
    let mut total = 0.0;
    for step in 0..SEGMENTS {
        let t = step as f32 / (SEGMENTS - 1) as f32;
        let x = -0.45 + t * 0.9;
        let y = -0.28 + t * 0.52;
        let z = (1.0f32 - x * x - y * y).max(0.05).sqrt();
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &[GestureSample {
                    position: [x, y, z],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("a dab");

        let started = Instant::now();
        geometry.sync(&harness.gpu, document).expect("sync");
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        total += ms;
        worst = worst.max(ms);
    }
    (worst, total)
}

#[test]
fn lifting_the_pointer_stays_inside_a_frame() {
    // The regression. `FRAME` is the application's own stall threshold — the
    // one that printed "a interface travou" — so this fails exactly when a
    // sculptor would have seen the message.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the first mesh");

    let (worst, total) = drag(&harness, &mut geometry, &mut document);

    // Everything the application does on `EndStroke` that is not the sync it
    // already did per segment. Only the coarse levels are left, and they
    // cannot be built mid-stroke: dirtying any child drops its mip.
    let started = Instant::now();
    document.build_mips().expect("build the mips");
    let pointer_up = started.elapsed();

    println!(
        "gesture: worst segment {worst:.2} ms, whole gesture {total:.2} ms, \
         pointer-up {:.2} ms",
        pointer_up.as_secs_f64() * 1000.0
    );

    assert!(
        pointer_up < FRAME,
        "the end of a gesture took {:.1} ms, over a {:.1} ms frame — which is \
         the hitch the shading pass used to cause. Something has been added \
         back onto pointer-up.",
        pointer_up.as_secs_f64() * 1000.0,
        FRAME.as_secs_f64() * 1000.0
    );
}
