//! What a gesture costs a frame, at both ends of it.
//!
//! Full-resolution SDF edits use gradient normals immediately. That avoids the
//! pitted face-normal rendering that used to remain until an idle refinement
//! pass, but it means the cost belongs to each moving frame.
//!
//! Lifting the pointer once ran a second shading pass over every key the stroke
//! had touched. The application reported it, in Portuguese, on every stroke:
//!
//! ```text
//! a interface travou: sombreamento final 17 ms
//! a interface travou: stroke 20 ms
//! ```
//!
//! Those two are one event: `stroke` is the outer timer around `EndStroke` and
//! the shading pass ran inside it. Shading fully during the drag removed it,
//! and moved the cost onto every frame of the drag instead — 40% on the median
//! segment, and a tail that reached 19 ms where face normals never left 6. The
//! cursor ring is drawn in the frame that meshes the edit, so that tail is
//! what a sculptor feels as the ring trailing the pointer.
//!
//! The current engine makes gradient sampling cheap enough to keep it in the
//! live path. Neither the live segments nor pointer-up may exceed a frame:
//! `FRAME` is the application's own stall threshold.
//!
//! `visual_incremental` and `visual_subtools` separately hold the live image
//! against a full rebuild, including its shading.

mod support;

use std::collections::HashSet;
use std::time::{Duration, Instant};

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind, FRAME};
use support::Harness;

/// Segments in the test gesture. Enough to exercise repeatedly overlapping
/// incremental remeshes rather than a single dab.
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

/// The triangles on screen, however they are split between keys.
///
/// Per-key equality is the wrong question — the engine attributes a straddling
/// triangle to the lowest *requested* key, so a subset re-mesh may legally move
/// one between keys. `settle_needed.rs` makes the same argument at more length.
fn triangles(geometry: &SurfaceGeometry) -> HashSet<[[i32; 3]; 3]> {
    geometry
        .stored_triangles()
        .into_values()
        .flatten()
        .collect()
}

#[test]
fn neither_end_of_a_gesture_leaves_the_frame() {
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
        pointer_up.as_secs_f64() * 1000.0,
    );

    // The budgets are a property of the binary that ships. Measured on one
    // machine, this gesture's worst segment runs 9.90 ms unoptimised against
    // 2.57 ms optimised — near enough four times — so a 16.7 ms bound in a
    // debug build measures the profile rather than the code. On a shared macOS
    // runner the same debug segment reads 21.0 ms, and this test failed every
    // CI run on that platform for weeks while the release step it was meant
    // for never got to run.
    //
    // The same guard `sculpt_latency` and `visual_brushes` already carry, and
    // the one the workflow assumes every timing test has: "The budgets are a
    // property of an optimised build. Measuring them in a debug build measures
    // the profile, so the timing assertions only run here." This was the test
    // that did not.
    //
    // Debug still does all the work above and prints the numbers; only the
    // verdict waits for a build that means something.
    if cfg!(debug_assertions) {
        println!(
            "  (debug build: timings reported, not asserted — \
             run with --release for the verdict)"
        );
    } else {
        // The regression the drag has to hold now that its gradient is sampled
        // immediately rather than deferred.
        assert!(
            Duration::from_secs_f64(worst / 1000.0) < FRAME,
            "the worst mid-drag segment took {worst:.1} ms, over a {:.1} ms frame. \
             The drag is meshing more, or shading more, than it can afford — the \
             ring is drawn in this same frame, so this is the pointer lag.",
            FRAME.as_secs_f64() * 1000.0
        );

        // The regression the end of a gesture has to hold.
        assert!(
            pointer_up < FRAME,
            "the end of a gesture took {:.1} ms, over a {:.1} ms frame — which is \
             the hitch the shading pass used to cause. Something has been added \
             back onto pointer-up.",
            pointer_up.as_secs_f64() * 1000.0,
            FRAME.as_secs_f64() * 1000.0
        );
    }
}

#[test]
fn an_incremental_gesture_has_the_same_triangles_as_a_rebuild() {
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

    drag(&harness, &mut geometry, &mut document);
    let incremental = triangles(&geometry);

    let mut rebuilt = SurfaceGeometry::new(&harness.gpu);
    rebuilt
        .rebuild(&harness.gpu, &mut document)
        .expect("rebuild");
    let whole = triangles(&rebuilt);

    let missing = whole.difference(&incremental).count();
    let extra = incremental.difference(&whole).count();
    println!(
        "incremental: {} triangles against a rebuild's {} — {missing} missing, {extra} spare",
        incremental.len(),
        whole.len(),
    );

    // Incremental replacement may not lose or invent a triangle.
    assert_eq!(
        missing, 0,
        "the incremental surface is missing {missing} triangles a rebuild has"
    );
    assert_eq!(
        extra, 0,
        "the incremental surface holds {extra} triangles a rebuild does not"
    );
}
