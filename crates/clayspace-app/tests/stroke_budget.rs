//! Where a stroke's milliseconds go.
//!
//! The headline number — a dab on the reference scene — is meaningless as a
//! target without knowing which term dominates it. This splits one segment
//! into the four things it does, so "can this be made interactive" is a
//! question about a specific call rather than about the application.
//!
//! Where it has stood, on this machine, at 80 bricks after 96 edits:
//!
//! | engine | edit | face normals | gradient normals |
//! |---|---|---|---|
//! | 0.28.0 | 1.09 ms | 7.71 ms | 83.22 ms |
//! | 0.29.0 | 0.97 ms | 7.86 ms | 19.90 ms |
//! | 0.29.1 | 0.95 ms | 8.02 ms | 11.48 ms |
//! | 0.30.0 | 1.00 ms | 12.49 ms | 13.66 ms |
//! | 0.39.0 | 0.52 ms |  6.26 ms |  9.46 ms |
//!
//! The table above is history rather than the record. Since the performance
//! gate grew to cover every operation, the figures live in
//! `benchmarks/baseline-<platform>.json` and `just bench-compare` is what reads
//! them across an engine pin. What this file is for is the *breakdown* — which
//! of the four terms a total is made of — which no single figure can say.
//!
//! Two upstream fixes, both to the same term: #73 culled the tape per brick,
//! and #83 batched the attribute taps through the CPU pool. The gradient has
//! gone from eleven times the cost of face normals to half again as much.
//!
//! Read the last column against the one beside it and not against the rows
//! above it. The face-normal column is not comparable across those rows — the
//! 0.30.0 row is the first taken from a build that links CUDA at all, so the
//! 96 dabs behind it were refilled by a different backend and the 80 bricks
//! sampled hold different geometry. What the row *does* say is that on the
//! same sample, on the same day, the gradient costs 1.1x face normals.
//!
//! 0.39.0 halves both terms — the batched brick refill (#204) and the meshing
//! work behind it — and widens the gradient's share back out to 1.5x, which is
//! where it was at 0.29.1. Upstream's own `clay_bench` says the same thing
//! about the call underneath: `MeshBricksWhole` 27.4 -> 6.97 ms,
//! `MeshBricksGradGrownDoc` 9.85 -> 4.36 ms.
//!
//! It does not follow that the drag can afford the gradient. This is a fixed
//! 80-brick sample; a segment meshes the 27 keys a dab dirtied, and over those
//! the premium was 40% at the median with a tail reaching 19 ms.
//! `gesture_end.rs` is where that is measured and held.

mod support;

use std::time::Instant;

use clayspace_app::{SharedDocument, SurfaceGeometry};
use clayspace_engine::claycore::BrickMeshParams;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

/// A document with a stroke's worth of history behind it, which is what makes
/// the measurement representative rather than best-case.
fn worked(document: &mut SharedDocument, edits: usize) {
    for step in 0..edits {
        let t = step as f32 / edits.max(1) as f32;
        let _ = document.apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: [(t - 0.5) * 0.8, (t * 5.0).sin() * 0.2, 1.0],
                pressure: 1.0,
                time: t,
            }],
            [false; 3],
        );
    }
}

#[test]
fn one_segment_split_into_what_it_spends() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form) else {
        return;
    };
    let mut document = SharedDocument::new(document);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    document
        .with(|d| geometry.rebuild(&harness.gpu, d))
        .expect("first mesh");

    worked(&mut document, 96);

    // The edit itself: apply the stroke, mark what it dirtied, refill those
    // bricks. Everything the *sculpting* costs.
    let started = Instant::now();
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: [0.0, 0.0, 1.02],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("a dab");
    let edit = started.elapsed();

    // Meshing the region that changed, both ways.
    let keys = document.with(|d| d.cache().surface_bricks().expect("bricks"));
    let sample: Vec<_> = keys.iter().copied().take(80).collect();

    let mesh_with = |gradient: bool| {
        let started = Instant::now();
        let _ = document.with(|d| {
            d.cache()
                .mesh(
                    Some(d.document()),
                    BrickMeshParams {
                        gradient_normals: gradient,
                        colors: false,
                        gradient_eps: None,
                    },
                    &sample,
                )
                .expect("mesh")
        });
        started.elapsed()
    };
    // Warm, so the first call's allocation is not attributed to normals.
    let _ = mesh_with(false);
    let flat = mesh_with(false);
    let gradient = mesh_with(true);

    let started = Instant::now();
    document
        .with(|d| geometry.sync(&harness.gpu, d))
        .expect("sync");
    let sync = started.elapsed();

    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    println!("\n--- one segment, after 96 edits, 80 bricks meshed ---");
    println!("  edit (stroke + mark + refill) : {:7.2} ms", ms(edit));
    println!("  mesh, normals off             : {:7.2} ms", ms(flat));
    println!("  mesh, gradient normals        : {:7.2} ms", ms(gradient));
    println!("  full sync (mesh + upload)     : {:7.2} ms", ms(sync));
    println!(
        "  gradient normals cost         : {:7.2} ms  ({:.1}x)",
        ms(gradient) - ms(flat),
        ms(gradient) / ms(flat).max(0.001)
    );

    // The claim this test exists to hold: sculpting is not what costs. If the
    // edit itself ever becomes the dominant term, the conclusion below — that
    // the latency is one upstream meshing call — stops being true.
    assert!(
        edit < gradient,
        "the edit ({:.2} ms) now costs more than meshing it ({:.2} ms); \
         the performance story has changed and the notes citing ClayCore #73 \
         need revisiting",
        ms(edit),
        ms(gradient)
    );
}
