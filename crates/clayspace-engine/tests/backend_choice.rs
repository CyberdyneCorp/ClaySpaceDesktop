//! Which backend refill should run on.
//!
//! `BackendPolicy::refill_backend` returns the CPU today because the
//! accelerated backends are slower at this verb — see ClayCore #64. That is a
//! measurement, not a principle, so it is checked rather than asserted in a
//! comment: when the engine's Metal path gets faster, this fails and the
//! choice should be revisited.

use std::time::{Duration, Instant};

use clayspace_engine::claycore::{
    Blend, BrickCache, BrickConfig, Document, Item, Op, StrokePreset, StrokeSample,
};
use clayspace_engine::BackendPolicy;

/// The configuration the application runs.
const CONFIG: BrickConfig = BrickConfig {
    dim: 8,
    voxel_size: 0.02,
    band_voxels: 3,
    memory_budget: Some(512 * 1024 * 1024),
    colors: false,
};

/// Median refill cost for one dab's worth of bricks.
fn dab_refill(backend: Option<&clayspace_engine::claycore::Backend>) -> Option<Duration> {
    let mut doc = Document::new().ok()?;
    let layer = doc.add_sdf_layer("L").ok()?;
    doc.add_item(layer, &Item::sphere(1.0).ok()?).ok()?;

    let mut cache = BrickCache::new(CONFIG).ok()?;
    cache.mark_dirty([-2.0; 3], [2.0; 3]).ok()?;
    cache.refill_all(&doc, None, 512).ok()?;

    let mut stamp = Item::sphere(0.18).ok()?;
    stamp.set_op(Op::Relief).ok()?;
    stamp.set_blend(Blend::Quadratic, 0.18).ok()?;
    stamp.set_rounding(0.18).ok()?;
    let preset = StrokePreset {
        radius: 0.18,
        ..Default::default()
    };

    let mut times = Vec::new();
    for i in 0..12 {
        let t = i as f32 / 11.0;
        let angle = (t - 0.5) * 1.2;
        let (s, c) = angle.sin_cos();
        let nodes = doc
            .apply_stroke(
                layer,
                &[StrokeSample {
                    position: [s * 1.01, 0.1, c * 1.01],
                    pressure: 1.0,
                    time: t,
                }],
                &preset,
                &stamp,
                None,
            )
            .ok()?;
        cache.mark_dirty_nodes(&doc, layer, &nodes).ok()?;
        let (requests, _) = cache.take_dirty(512).ok()?;
        let started = Instant::now();
        cache.refill(&doc, backend, &requests).ok()?;
        times.push(started.elapsed());
    }
    times.sort();
    Some(times[times.len() / 2])
}

#[test]
fn refill_runs_on_whichever_backend_is_actually_faster() {
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let accelerated = policy.active().clone();
    if accelerated == clayspace_engine::claycore::Backend::Cpu {
        // Nothing to compare against on this machine.
        return;
    }

    let (Some(cpu), Some(gpu)) = (dab_refill(None), dab_refill(Some(&accelerated))) else {
        return;
    };
    println!(
        "refill per dab: cpu {:.2} ms, {accelerated} {:.2} ms",
        cpu.as_secs_f64() * 1000.0,
        gpu.as_secs_f64() * 1000.0
    );

    let chosen_is_cpu = policy.refill_backend().is_none();
    if cpu <= gpu {
        assert!(
            chosen_is_cpu,
            "the CPU refills a dab in {cpu:?} against {accelerated}'s {gpu:?}, \
             and yet refill is routed to {accelerated}"
        );
    } else {
        assert!(
            !chosen_is_cpu,
            "{accelerated} now refills a dab in {gpu:?} against the CPU's \
             {cpu:?} — ClayCore #64 is fixed, so `refill_backend` should hand \
             the work back to it"
        );
    }
}

#[test]
fn the_active_backend_is_still_reported_honestly() {
    // Routing refill to the CPU must not turn the status bar into a lie in the
    // other direction: `active()` reports what the machine offers and what
    // other verbs use, and stays true whatever this one verb does.
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    assert!(
        policy.available().contains(policy.active()),
        "the active backend is not one this machine offers"
    );
}
