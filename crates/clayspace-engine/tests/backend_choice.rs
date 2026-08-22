//! Which backend refill should run on.
//!
//! `BackendPolicy::refill_backend` routes by batch size. Before ClayCore
//! 0.28.0 it refused the accelerated backends outright, because the Metal path
//! paid a full device round trip per brick and sat 7–10× behind the CPU at
//! every size (#64). Batched into one dispatch it is now roughly twice as fast
//! at a dab, so the decision became a threshold.
//!
//! That is a measurement, not a principle, and it is checked rather than
//! asserted in a comment: this fails if the ratio flips back, or if the
//! threshold stops matching what the machine actually does.

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

/// A dab is about this many bricks, which is over the threshold — so this is
/// the batch size the routing decision is really about.
const DAB: usize = 27;

#[test]
fn refill_runs_on_whichever_backend_is_actually_faster() {
    // The routing used to be a constant measured on one machine, and this test
    // is what caught it being wrong on another: on a 24-thread Linux box with
    // an RTX 5060 the CPU wins at every batch size, and the constant sent a
    // dab to the GPU anyway. It is now measured, so what this asserts is that
    // the policy agrees with the measurement it was given.
    let Ok(mut policy) = BackendPolicy::discover(None) else {
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

    // The same batch size for both, so comparing per-brick costs is comparing
    // these two durations.
    policy.record_refill(None, DAB, cpu);
    policy.record_refill(Some(&accelerated), DAB, gpu);

    let chosen_is_cpu = policy.refill_backend(DAB).is_none();
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
             {cpu:?}, and yet refill is still routed to the CPU"
        );
    }
}

#[test]
fn routing_follows_the_measurement_rather_than_the_constant() {
    // The property that makes the constant survivable: whatever it says, one
    // measurement each way settles it. Driven with synthetic timings so it
    // asserts the policy's own logic on every machine, including CPU-only
    // ones where there is nothing to race.
    let Ok(mut policy) = BackendPolicy::discover(None) else {
        return;
    };
    let accelerated = policy.active().clone();
    if accelerated == clayspace_engine::claycore::Backend::Cpu {
        return;
    }

    // Before anything is measured the constant decides, which is what produces
    // the first sample.
    assert!(
        policy.refill_backend(DAB).is_some(),
        "an unmeasured policy should try the accelerated backend"
    );
    assert!(policy.needs_refill_calibration());

    // A slow accelerated backend sends the work to the CPU.
    policy.record_refill(None, DAB, Duration::from_micros(500));
    policy.record_refill(Some(&accelerated), DAB, Duration::from_micros(2000));
    assert!(!policy.needs_refill_calibration());
    assert!(
        policy.refill_backend(DAB).is_none(),
        "a backend measured four times slower than the CPU still got the work"
    );

    // And the other way, which is the machine the constant was written for.
    let Ok(mut policy) = BackendPolicy::discover(None) else {
        return;
    };
    policy.record_refill(None, DAB, Duration::from_micros(2000));
    policy.record_refill(Some(&accelerated), DAB, Duration::from_micros(500));
    assert!(
        policy.refill_backend(DAB).is_some(),
        "a backend measured four times faster than the CPU did not get the work"
    );
}

#[test]
fn a_measured_policy_still_keeps_small_batches_on_the_cpu() {
    // The guard that is not about throughput: what a handful of residual
    // bricks avoids is the fixed cost of a device submission, which no
    // per-brick measurement of a large batch can see.
    let Ok(mut policy) = BackendPolicy::discover(None) else {
        return;
    };
    let accelerated = policy.active().clone();
    if accelerated == clayspace_engine::claycore::Backend::Cpu {
        return;
    }
    // Measured as overwhelmingly favouring the accelerated backend.
    policy.record_refill(None, DAB, Duration::from_micros(10_000));
    policy.record_refill(Some(&accelerated), DAB, Duration::from_micros(100));

    assert!(
        policy.refill_backend(1).is_none(),
        "one brick was sent to a device"
    );
    assert!(
        policy
            .refill_backend(BackendPolicy::GPU_CROSSOVER_BRICKS - 1)
            .is_none(),
        "a sub-threshold batch was sent to a device"
    );
}

#[test]
fn a_handful_of_residual_bricks_stays_on_the_cpu() {
    // The other half of the threshold. A device submission costs about
    // 0.25 ms whatever it carries, and a stroke's last drain is often a few
    // bricks — which the CPU does in a hundredth of that.
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    for bricks in [0, 1, 4, BackendPolicy::GPU_CROSSOVER_BRICKS - 1] {
        assert!(
            policy.refill_backend(bricks).is_none(),
            "{bricks} bricks were routed off the CPU, under a threshold of {}",
            BackendPolicy::GPU_CROSSOVER_BRICKS
        );
    }
}

#[test]
fn a_cpu_only_machine_is_never_routed_anywhere_else() {
    // There is nowhere else to route to, and asking for a backend the machine
    // does not have would be an error rather than a slow path.
    let policy =
        BackendPolicy::from_available(vec![clayspace_engine::claycore::Backend::Cpu], None);
    assert!(policy.refill_backend(100_000).is_none());
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

#[test]
fn a_real_document_calibrates_itself_on_its_first_big_refill() {
    // The end-to-end half. The split happens inside `drain_dirty`, so building
    // a starting form — which refills the whole thing — is enough to leave the
    // policy measured rather than guessing.
    //
    // This is what took startup on a 24-thread Linux box with an RTX 5060 from
    // 179 ms to 63 ms: the constant sent the whole fill to a backend four times
    // slower than the CPU, and two 32-brick slices are enough to find that out.
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    if policy.active() == &clayspace_engine::claycore::Backend::Cpu {
        return;
    }
    let Ok(document) = clayspace_engine::ClayDocument::new(policy)
        .and_then(clayspace_engine::ClayDocument::with_starting_form)
    else {
        return;
    };

    let (cpu, accelerated) = document.policy().refill_cost_per_brick();
    assert!(
        cpu.is_some() && accelerated.is_some(),
        "the first full refill left the routing unmeasured: cpu {cpu:?}, \
         accelerated {accelerated:?}"
    );
    assert!(
        !document.policy().needs_refill_calibration(),
        "the policy still wants calibrating after a whole-model refill"
    );
    println!(
        "per brick: cpu {:.0} ns, {} {:.0} ns",
        cpu.unwrap_or(0.0),
        document.policy().active(),
        accelerated.unwrap_or(0.0)
    );
}
