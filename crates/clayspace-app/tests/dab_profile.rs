//! Where a dab's milliseconds go.
//!
//! Not a pass/fail test — `sculpt_latency` owns the budget. This exists to
//! answer "why is it still slow" with a breakdown rather than a total, so the
//! next optimisation is aimed at the stage that actually costs something.
//!
//! ```sh
//! cargo test -p clayspace-app --test dab_profile --release -- --nocapture
//! ```

mod support;

use std::time::{Duration, Instant};

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

/// A drag across the front of the form, sampled as a pointer would deliver it.
fn path(steps: usize) -> Vec<[f32; 3]> {
    (0..steps)
        .map(|i| {
            let t = i as f32 / (steps - 1) as f32;
            let angle = (t - 0.5) * 1.2;
            let (s, c) = angle.sin_cos();
            [s * 1.01, 0.1, c * 1.01]
        })
        .collect()
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

#[test]
fn where_a_dab_spends_its_time() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("first mesh");

    let brush = BrushSettings::default();
    let mut edit_times = Vec::new();
    let mut costs = Vec::new();

    for (i, position) in path(24).iter().enumerate() {
        let samples = [GestureSample {
            position: *position,
            pressure: 1.0,
            time: i as f32 * 0.01,
        }];

        // Stage one: the engine applies the stroke and refills the bricks it
        // dirtied. Nothing of ours runs inside this.
        let started = Instant::now();
        document
            .apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])
            .expect("stroke");
        edit_times.push(started.elapsed());

        // Stage two: turning those bricks into something the GPU can draw.
        if let Some(cost) = geometry
            .sync(&harness.gpu, &mut document)
            .expect("re-mesh")
        {
            costs.push(cost);
        }
    }

    if costs.is_empty() {
        return;
    }

    let median = |mut values: Vec<f64>| {
        values.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        values[values.len() / 2]
    };

    let edit = median(edit_times.iter().map(|d| ms(*d)).collect());
    let engine_mesh = median(costs.iter().map(|c| ms(c.engine_mesh_time)).collect());
    let read = median(costs.iter().map(|c| ms(c.read_time)).collect());
    let split = median(costs.iter().map(|c| ms(c.split_time)).collect());
    let upload = median(costs.iter().map(|c| ms(c.upload_time)).collect());
    let total = edit + engine_mesh + read + split + upload;

    let keys = median(costs.iter().map(|c| c.keys as f64).collect());
    let triangles = median(costs.iter().map(|c| c.triangles as f64).collect());

    println!("\nmedian dab over {} samples", costs.len());
    println!("  {keys:.0} keys re-meshed, {triangles:.0} triangles in the buffer\n");
    let row = |name: &str, value: f64| {
        println!(
            "  {name:<34} {value:>7.1} ms   {:>5.1}%",
            value / total * 100.0
        );
    };
    row("engine: apply_stroke + refill", edit);
    row("engine: brick cache mesh", engine_mesh);
    row("ours:   copy into vertex layout", read);
    row("ours:   split into per-key geometry", split);
    row("ours:   concatenate and upload", upload);
    println!("  {:<34} {total:>7.1} ms", "total");
    println!(
        "\n  engine {:.0}%, ours {:.0}%\n",
        (edit + engine_mesh) / total * 100.0,
        (read + split + upload) / total * 100.0
    );

    // The one thing worth failing on: the buffer is rebuilt whole every dab,
    // so if that ever dominates, the incremental path has stopped being
    // incremental in the only place it still is.
    assert!(
        upload < total * 0.5,
        "uploading is {:.0}% of a dab, which is not what this path is for",
        upload / total * 100.0
    );
}
