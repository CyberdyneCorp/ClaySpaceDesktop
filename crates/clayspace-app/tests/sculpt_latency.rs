//! What a brush dab costs, measured against the runner's fixed control.
//!
//! The specification puts a 50 ms median and 100 ms 95th percentile on
//! input-to-visible with a GPU backend active. This is the part of that budget
//! the application controls: meshing what the edit dirtied and getting it onto
//! the GPU. It also writes a capture of the result, so the surface can be
//! looked at as well as timed.

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

/// The reference-machine product targets are 50 ms median and 100 ms p95.
/// CI uses these fractions of a full rebuild of the same fixed scene, so a
/// shared runner's raw speed does not decide whether the dab path regressed.
const MEDIAN_REBUILD_FRACTION: u32 = 5;
const P95_REBUILD_FRACTION: u32 = 3;

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("backends");
    ClayDocument::new(policy)
        .expect("document")
        .with_starting_form()
        .expect("starting form")
}

/// A short dab across the surface, as a drag would deliver it.
fn dab(index: usize) -> Vec<GestureSample> {
    let angle = -0.6 + index as f32 * 0.05;
    (0..4)
        .map(|i| {
            let t = angle + i as f32 * 0.012;
            let (s, c) = t.sin_cos();
            GestureSample {
                position: [s * 1.01, 0.1, c * 1.01],
                pressure: 1.0,
                time: i as f32 * 0.008,
            }
        })
        .collect()
}

#[test]
fn a_dab_meshes_only_what_it_dirtied() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let mut document = document();
    let mut geometry = SurfaceGeometry::new(&harness.gpu);

    // The first sync builds everything the starting form covers.
    let initial = geometry
        .sync(&harness.gpu, &mut document)
        .expect("initial sync")
        .expect("the starting form is dirty");
    assert!(initial.keys > 0, "the starting form produced no keys");

    // A dab then dirties a fraction of it.
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &dab(0),
            [false; 3],
        )
        .expect("dab");

    let after = geometry
        .sync(&harness.gpu, &mut document)
        .expect("sync")
        .expect("the dab is dirty");

    assert!(
        after.keys < initial.keys,
        "a dab re-meshed {} keys against the form's {}, so the cost is not bounded \
         by what the edit touched",
        after.keys,
        initial.keys
    );
    assert!(after.triangles > 0, "the surface lost its geometry");
}

#[test]
fn a_frame_with_no_edit_costs_nothing() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let mut document = document();
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.sync(&harness.gpu, &mut document).expect("initial");

    for _ in 0..10 {
        assert!(
            geometry
                .sync(&harness.gpu, &mut document)
                .expect("sync")
                .is_none(),
            "an idle frame re-meshed something"
        );
    }
}

#[test]
fn dab_latency_stays_inside_the_budget() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let mut document = document();
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.sync(&harness.gpu, &mut document).expect("initial");

    let mut timings = Vec::new();
    for index in 0..24 {
        let started = std::time::Instant::now();
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &dab(index),
                [false; 3],
            )
            .expect("dab");
        geometry.sync(&harness.gpu, &mut document).expect("sync");
        timings.push(started.elapsed());
    }

    timings.sort();
    let median = timings[timings.len() / 2];
    let p95 = timings[(timings.len() * 95) / 100];

    let mut reference = SurfaceGeometry::new(&harness.gpu);
    let started = std::time::Instant::now();
    reference
        .rebuild(&harness.gpu, &mut document)
        .expect("reference full rebuild");
    let reference_rebuild = started.elapsed();

    // Reported whether or not it passes, because a number is more useful than
    // a verdict when the budget is the thing being designed against.
    println!(
        "dab latency over {} dabs: median {:.1} ms, p95 {:.1} ms, worst {:.1} ms, \
         reference full rebuild {:.1} ms",
        timings.len(),
        median.as_secs_f64() * 1000.0,
        p95.as_secs_f64() * 1000.0,
        timings.last().unwrap().as_secs_f64() * 1000.0,
        reference_rebuild.as_secs_f64() * 1000.0,
    );
    println!(
        "  shares of rebuild: median {:.1}%, p95 {:.1}%",
        median.as_secs_f64() / reference_rebuild.as_secs_f64() * 100.0,
        p95.as_secs_f64() / reference_rebuild.as_secs_f64() * 100.0,
    );
    if let Some(cost) = geometry.last_cost() {
        println!(
            "  last sync: {} keys, mesh {:.2} ms, upload {:.2} ms, {} triangles",
            cost.keys,
            cost.mesh_time.as_secs_f64() * 1000.0,
            cost.upload_time.as_secs_f64() * 1000.0,
            cost.triangles
        );
    }

    // The ratio is a property of the binary that ships. An unoptimised build
    // measures the profile rather than the code, so debug reports the numbers
    // and release gives the verdict. The full rebuild is fixed work on the
    // same scene and runner, while a dab should remain a small fraction of it.
    if cfg!(debug_assertions) {
        println!(
            "  (debug build: timing ratios reported, not asserted — \
             run with --release for the verdict)"
        );
    } else {
        assert!(
            median <= reference_rebuild / MEDIAN_REBUILD_FRACTION,
            "median dab latency {:.1} ms exceeds one fifth of the {:.1} ms \
             full rebuild",
            median.as_secs_f64() * 1000.0,
            reference_rebuild.as_secs_f64() * 1000.0,
        );
        assert!(
            p95 <= reference_rebuild / P95_REBUILD_FRACTION,
            "95th percentile dab latency {:.1} ms exceeds one third of the \
             {:.1} ms full rebuild",
            p95.as_secs_f64() * 1000.0,
            reference_rebuild.as_secs_f64() * 1000.0,
        );
    }

    // And the result is worth looking at, not only timing.
    let camera = {
        let mut camera = clayspace_view::Camera::default();
        match document.bounds() {
            Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
            None => camera.frame_default(),
        }
        camera
    };
    harness.capture(geometry.mesh(), &camera, false, "50-sculpted-surface");
}

#[test]
fn compaction_rebuilds_the_surface_without_changing_it() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let mut document = document();
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.sync(&harness.gpu, &mut document).expect("initial");

    for index in 0..6 {
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &dab(index),
                [false; 3],
            )
            .expect("dab");
        geometry.sync(&harness.gpu, &mut document).expect("sync");
    }

    let camera = {
        let mut camera = clayspace_view::Camera::default();
        match document.bounds() {
            Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
            None => camera.frame_default(),
        }
        camera
    };
    let before = harness.capture(geometry.mesh(), &camera, false, "51-before-compaction");

    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("compaction");
    let after = harness.capture(geometry.mesh(), &camera, false, "51-after-compaction");

    let background = harness.background();
    let moved = before.changed_fraction_over_subject(&after, background, 8);
    assert!(
        moved < 0.05,
        "compaction changed {:.1}% of the surface; it must reclaim space without \
         altering what is drawn",
        moved * 100.0
    );
}

#[test]
fn full_rebuilds_keep_sparse_uploads_while_compaction_patches() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let mut document = document();
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.sync(&harness.gpu, &mut document).unwrap();
    harness.gpu.take_uploaded_bytes();
    geometry.rebuild(&harness.gpu, &mut document).unwrap();
    let sparse_bytes = (geometry.vertex_count() * clayspace_view::Vertex::STRIDE) as u64
        + u64::from(geometry.mesh().index_count()) * 4;
    assert_eq!(
        harness.gpu.take_uploaded_bytes(),
        sparse_bytes,
        "full rebuilds must not transfer unused vertex gaps"
    );

    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &dab(0),
            [false; 3],
        )
        .unwrap();
    geometry.sync(&harness.gpu, &mut document).unwrap();
    harness.gpu.take_uploaded_bytes();
    geometry
        .settle_after_edit(&harness.gpu, &mut document)
        .unwrap();
    let live_bytes = (geometry.vertex_count() * clayspace_view::Vertex::STRIDE) as u64
        + u64::from(geometry.mesh().index_count()) * 4;
    // Compaction rewrites only the keys it changed (issue #175); the mapped
    // whole-surface layout is its fallback, not its default.
    let compacted = harness.gpu.take_uploaded_bytes();
    assert!(
        compacted < live_bytes / 4,
        "compaction uploaded {compacted} bytes of a {live_bytes}-byte surface; \
         it must patch the keys it changed rather than relay the layer out"
    );
}
