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
//! live path. A shared runner can miss the application's frame threshold when
//! unrelated work takes the machine, so the release gate measures each end
//! against a fixed full rebuild of this same scene in the same process.
//!
//! `visual_incremental` and `visual_subtools` separately hold the live image
//! against a full rebuild, including its shading.

mod support;

use std::collections::HashSet;
use std::time::{Duration, Instant};

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

/// Segments in the test gesture. Enough to exercise repeatedly overlapping
/// incremental remeshes rather than a single dab.
const SEGMENTS: usize = 24;
/// On the reference machine a segment takes about 4% of a full rebuild.
/// One quarter leaves room for noise while catching a lost incremental path.
const SEGMENT_REBUILD_FRACTION: u32 = 4;
/// Pointer-up takes about 2% of a rebuild. One tenth catches a second shading
/// pass returning to the end of the gesture.
const END_REBUILD_FRACTION: u32 = 10;

fn within_reference_fraction(observed: Duration, reference: Duration, denominator: u32) -> bool {
    observed <= reference / denominator
}

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
) -> (Duration, Duration) {
    let mut worst = Duration::ZERO;
    let mut total = Duration::ZERO;
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
        let elapsed = started.elapsed();
        total += elapsed;
        worst = worst.max(elapsed);
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

    let mut reference = SurfaceGeometry::new(&harness.gpu);
    let started = Instant::now();
    reference
        .rebuild(&harness.gpu, &mut document)
        .expect("reference full rebuild");
    let reference_rebuild = started.elapsed();

    println!(
        "gesture: worst segment {:.2} ms, whole gesture {:.2} ms, \
         pointer-up {:.2} ms, reference full rebuild {:.2} ms",
        worst.as_secs_f64() * 1000.0,
        total.as_secs_f64() * 1000.0,
        pointer_up.as_secs_f64() * 1000.0,
        reference_rebuild.as_secs_f64() * 1000.0,
    );
    println!(
        "  shares of rebuild: segment {:.1}%, pointer-up {:.1}%",
        worst.as_secs_f64() / reference_rebuild.as_secs_f64() * 100.0,
        pointer_up.as_secs_f64() / reference_rebuild.as_secs_f64() * 100.0,
    );

    // The ratio is a property of the optimised binary. Debug still does all
    // the work and prints the numbers; only the verdict waits for release.
    // A fixed full rebuild is the control workload: if an incremental segment
    // starts rebuilding most of the scene, or pointer-up starts shading it
    // again, it approaches this control no matter how fast the runner is.
    if cfg!(debug_assertions) {
        println!(
            "  (debug build: timing ratios reported, not asserted — \
             run with --release for the verdict)"
        );
    } else {
        assert!(
            within_reference_fraction(worst, reference_rebuild, SEGMENT_REBUILD_FRACTION),
            "the worst mid-drag segment took {:.1} ms against a {:.1} ms full \
             rebuild (limit: one quarter). The drag is losing its incremental \
             meshing or shading path.",
            worst.as_secs_f64() * 1000.0,
            reference_rebuild.as_secs_f64() * 1000.0,
        );

        assert!(
            within_reference_fraction(pointer_up, reference_rebuild, END_REBUILD_FRACTION),
            "the end of a gesture took {:.1} ms against a {:.1} ms full rebuild \
             (limit: one tenth). A shading pass may have returned to pointer-up.",
            pointer_up.as_secs_f64() * 1000.0,
            reference_rebuild.as_secs_f64() * 1000.0,
        );
    }
}

#[test]
fn the_reference_gate_scales_with_the_runner_and_catches_a_lost_incremental_path() {
    let ms = Duration::from_millis;
    assert!(within_reference_fraction(
        ms(2),
        ms(20),
        SEGMENT_REBUILD_FRACTION
    ));
    assert!(within_reference_fraction(
        ms(20),
        ms(200),
        SEGMENT_REBUILD_FRACTION
    ));
    assert!(!within_reference_fraction(
        ms(20),
        ms(40),
        SEGMENT_REBUILD_FRACTION
    ));
    assert!(within_reference_fraction(
        ms(1),
        ms(20),
        END_REBUILD_FRACTION
    ));
    assert!(!within_reference_fraction(
        ms(5),
        ms(20),
        END_REBUILD_FRACTION
    ));
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
