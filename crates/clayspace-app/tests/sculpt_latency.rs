//! What a brush dab costs, measured rather than asserted.
//!
//! The specification puts a 50 ms median and 100 ms 95th percentile on
//! input-to-visible with a GPU backend active. This is the part of that budget
//! the application controls: meshing what the edit dirtied and getting it onto
//! the GPU. It also writes a capture of the result, so the surface can be
//! looked at as well as timed.

mod support;

use std::time::Duration;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

/// The specification's median budget for input to visible.
const MEDIAN_BUDGET: Duration = Duration::from_millis(50);
/// And its ninety-fifth percentile.
const P95_BUDGET: Duration = Duration::from_millis(100);

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

    // Reported whether or not it passes, because a number is more useful than
    // a verdict when the budget is the thing being designed against.
    println!(
        "dab latency over {} dabs: median {:.1} ms, p95 {:.1} ms, worst {:.1} ms",
        timings.len(),
        median.as_secs_f64() * 1000.0,
        p95.as_secs_f64() * 1000.0,
        timings.last().unwrap().as_secs_f64() * 1000.0
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

    // The budget is a property of the binary that ships. An unoptimised
    // build runs this work about two and a half times slower, so asserting a
    // real-time bound against it measures the profile rather than the code —
    // and the pressure that creates is to loosen the budget or to undo a
    // correctness fix to fit it. Debug still runs everything above and prints
    // the numbers; only the verdict is held for a build that means something.
    if cfg!(debug_assertions) {
        println!(
            "  (debug build: timings reported, not asserted — \
             run with --release for the verdict)"
        );
    } else {
        assert!(
            median <= MEDIAN_BUDGET,
            "median dab latency {:.1} ms exceeds the {} ms budget",
            median.as_secs_f64() * 1000.0,
            MEDIAN_BUDGET.as_millis()
        );
        assert!(
            p95 <= P95_BUDGET,
            "95th percentile dab latency {:.1} ms exceeds the {} ms budget",
            p95.as_secs_f64() * 1000.0,
            P95_BUDGET.as_millis()
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
