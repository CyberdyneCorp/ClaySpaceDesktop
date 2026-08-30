//! What `clay_brick_cache_mesh` costs, and why.
//!
//! The dab profile says two thirds of a stroke sample is this one call. Before
//! asking ClayCore for anything, three questions have to be separated:
//!
//!   1. Does the cost scale with the keys asked for, or is there a fixed
//!      overhead per call? If a 27-key mesh costs what a 343-key mesh costs,
//!      no amount of tightening our dirty set will help and the ask is theirs.
//!   2. How much of it is gradient normals? Those need the document and a
//!      compiled tape; face normals need neither. If they dominate, the ask is
//!      ours — stop asking for them.
//!   3. How much of the *rest* of a dab is applying the stroke versus
//!      refilling the bricks it dirtied?
//!
//! ```sh
//! cargo test -p clayspace-app --test mesh_scaling --release -- --nocapture
//! ```

use std::time::{Duration, Instant};

use clayspace_engine::claycore::{
    Blend, BrickCache, BrickConfig, BrickMeshParams, Document, Item, LayerId, Op, StrokePreset,
    StrokeSample,
};

const CONFIG: BrickConfig = BrickConfig {
    dim: 8,
    voxel_size: 0.02,
    band_voxels: 3,
    memory_budget: Some(512 * 1024 * 1024),
    colors: false,
};

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Best of several runs, so a stray scheduling hiccup does not become a claim.
fn best_of<T>(runs: usize, mut work: impl FnMut() -> T) -> Duration {
    let mut best = Duration::MAX;
    for _ in 0..runs {
        let started = Instant::now();
        let value = work();
        let elapsed = started.elapsed();
        std::hint::black_box(value);
        best = best.min(elapsed);
    }
    best
}

fn sphere() -> (Document, LayerId) {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("L").expect("layer");
    doc.add_item(layer, &Item::sphere(1.0).expect("sphere"))
        .expect("add");
    (doc, layer)
}

fn filled(doc: &Document) -> BrickCache {
    let mut cache = BrickCache::new(CONFIG).expect("cache");
    cache.mark_dirty([-2.0; 3], [2.0; 3]).expect("mark");
    cache.refill_all(doc, None, 512).expect("fill");
    cache
}

fn stamp(radius: f32) -> Item {
    let mut stamp = Item::sphere(radius).expect("stamp");
    stamp.set_op(Op::Relief).expect("op");
    stamp.set_blend(Blend::Quadratic, radius).expect("blend");
    stamp.set_rounding(radius).expect("rounding");
    stamp
}

#[test]
fn what_the_brick_mesh_call_costs() {
    let (doc, _) = sphere();
    let cache = filled(&doc);
    let all = cache.surface_bricks().expect("surface bricks");
    println!("\nsurface bricks on a unit sphere: {}", all.len());

    let gradient = BrickMeshParams {
        gradient_normals: true,
        colors: false,
        gradient_eps: None,
    };
    let plain = BrickMeshParams {
        gradient_normals: false,
        colors: false,
        gradient_eps: None,
    };

    println!("\n--- cost against key count ---");
    println!(
        "  {:>7} {:>14} {:>14} {:>10} {:>12}",
        "keys", "gradient ms", "no-normals ms", "grad/key", "gradient x"
    );

    let mut rows = Vec::new();
    for count in [1usize, 8, 27, 64, 125, 343] {
        if count > all.len() {
            continue;
        }
        let keys: Vec<_> = all.iter().copied().take(count).collect();
        let with = best_of(5, || {
            cache
                .mesh(Some(&doc), gradient, &keys)
                .expect("mesh")
                .0
                .vertex_count()
        });
        // Without a document there is no tape to sample, so no gradient
        // normals — this is the same marching with the expensive attribute
        // switched off.
        let without = best_of(5, || {
            cache
                .mesh(None, plain, &keys)
                .expect("mesh")
                .0
                .vertex_count()
        });
        println!(
            "  {count:>7} {:>14.2} {:>14.2} {:>10.3} {:>11.1}x",
            ms(with),
            ms(without),
            ms(with) / count as f64,
            ms(with) / ms(without).max(1e-6)
        );
        rows.push((count, ms(with), ms(without)));
    }

    // The whole surface, for scale.
    let whole = best_of(3, || {
        cache
            .mesh(Some(&doc), gradient, &[])
            .expect("mesh")
            .0
            .vertex_count()
    });
    println!(
        "  {:>7} {:>14.2}  (all surface bricks)",
        all.len(),
        ms(whole)
    );

    // Fixed overhead: what a one-key mesh costs against the per-key slope
    // measured at the top of the range.
    if let (Some(first), Some(last)) = (rows.first(), rows.last()) {
        let slope = (last.1 - first.1) / (last.0 - first.0).max(1) as f64;
        let fixed = first.1 - slope * first.0 as f64;
        println!("\n  per-key slope {slope:.3} ms, fixed overhead {fixed:.2} ms per call");
        println!(
            "  a 27-key dab would cost {:.2} ms of marching plus {fixed:.2} ms of overhead",
            slope * 27.0
        );
    }
}

#[test]
fn what_applying_a_stroke_costs() {
    // The other 21%: the stroke itself, then refilling what it dirtied.
    let (mut doc, layer) = sphere();
    let mut cache = filled(&doc);

    let preset = StrokePreset {
        radius: 0.18,
        ..Default::default()
    };
    let item = stamp(0.18);

    let mut apply = Duration::ZERO;
    let mut mark = Duration::ZERO;
    let mut refill = Duration::ZERO;
    let mut keys = 0usize;
    let samples = 16;

    for i in 0..samples {
        let t = i as f32 / (samples - 1) as f32;
        let angle = (t - 0.5) * 1.2;
        let (s, c) = angle.sin_cos();
        let position = [s * 1.01, 0.1, c * 1.01];

        let started = Instant::now();
        let nodes = doc
            .apply_stroke(
                layer,
                &[StrokeSample {
                    position,
                    pressure: 1.0,
                    time: t,
                }],
                &preset,
                &item,
                clayspace_engine::claycore::MaskSource::None,
            )
            .expect("stroke");
        apply += started.elapsed();

        let started = Instant::now();
        cache.mark_dirty_nodes(&doc, layer, &nodes).expect("mark");
        let (requests, _) = cache.take_dirty(512).expect("drain");
        mark += started.elapsed();

        keys += requests.len();
        let started = Instant::now();
        cache.refill(&doc, None, &requests).expect("refill");
        refill += started.elapsed();
    }

    let n = samples as f64;
    println!("\n--- a dab's engine-side edit, averaged over {samples} ---");
    println!("  clay_layer_apply_stroke        {:>7.2} ms", ms(apply) / n);
    println!("  mark_dirty_nodes + take_dirty  {:>7.2} ms", ms(mark) / n);
    println!(
        "  clay_brick_cache_refill        {:>7.2} ms",
        ms(refill) / n
    );
    println!("  bricks dirtied per dab         {:>7.1}", keys as f64 / n);
    println!(
        "  total                          {:>7.2} ms\n",
        ms(apply + mark + refill) / n
    );
}

#[test]
fn what_our_adapter_adds_on_top() {
    // The dab profile put "engine: apply_stroke + refill" at 7.9 ms, and the
    // raw sequence above measures 1.05 ms for the same work. The difference is
    // ours, and worth naming rather than leaving inside a bar labelled
    // "engine".
    use clayspace_engine::{BackendPolicy, ClayDocument};
    use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };

    let brush = BrushSettings::default();
    let mut total = Duration::ZERO;
    let samples = 16;
    for i in 0..samples {
        let t = i as f32 / (samples - 1) as f32;
        let angle = (t - 0.5) * 1.2;
        let (s, c) = angle.sin_cos();
        let started = Instant::now();
        document
            .apply_stroke(
                ToolKind::Padrao,
                brush,
                &[GestureSample {
                    position: [s * 1.01, 0.1, c * 1.01],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("stroke");
        total += started.elapsed();
        document.take_dirty_keys();
    }

    // And the cost of building the stamp item, which we do once per dab.
    let build = best_of(200, || {
        let mut item = Item::sphere(0.18).expect("stamp");
        item.set_op(Op::Relief).expect("op");
        item.set_blend(Blend::Quadratic, 0.18).expect("blend");
        item.set_rounding(0.18).expect("rounding");
        item
    });

    println!("\n--- our adapter, per dab ---");
    println!(
        "  ClayDocument::apply_stroke   {:>7.2} ms",
        ms(total) / samples as f64
    );
    println!("  ...of which building a stamp {:>7.2} ms", ms(build));
    println!("  raw engine sequence above    {:>7.2} ms\n", 1.05);
}

#[test]
fn what_the_backend_costs_for_a_small_refill() {
    // A dab dirties ~27 bricks. That is a small amount of work to hand to a
    // GPU, and dispatch is not free — so the backend that is faster for a
    // whole-model refill may be slower for a dab.
    use clayspace_engine::BackendPolicy;

    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let backend = policy.active().clone();
    println!("\n--- refill cost by backend ---");
    println!("  active backend: {backend}");

    for (label, use_backend) in [("cpu (None)", false), ("active", true)] {
        let (mut doc, layer) = sphere();
        let mut cache = filled(&doc);
        let preset = StrokePreset {
            radius: 0.18,
            ..Default::default()
        };
        let item = stamp(0.18);

        let mut refill = Duration::ZERO;
        let mut per_dab: Vec<f64> = Vec::new();
        let mut keys = 0usize;
        let samples = 16;
        for i in 0..samples {
            let t = i as f32 / (samples - 1) as f32;
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
                    &item,
                    clayspace_engine::claycore::MaskSource::None,
                )
                .expect("stroke");
            cache.mark_dirty_nodes(&doc, layer, &nodes).expect("mark");
            let (requests, _) = cache.take_dirty(512).expect("drain");
            keys += requests.len();
            let started = Instant::now();
            cache
                .refill(&doc, use_backend.then_some(&backend), &requests)
                .expect("refill");
            refill += started.elapsed();
            per_dab.push(ms(started.elapsed()));
        }
        per_dab.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        println!(
            "  {label:<12} mean {:>7.2}  first {:>7.2}  median {:>7.2}  best {:>7.2} ms  over {:.0} bricks",
            ms(refill) / samples as f64,
            per_dab.first().copied().unwrap_or(0.0).max(0.0),
            per_dab[per_dab.len() / 2],
            per_dab[0],
            keys as f64 / samples as f64
        );
    }

    // And the same comparison for a whole-model refill, where the GPU should
    // be the one that wins.
    for (label, use_backend) in [("cpu (None)", false), ("active", true)] {
        let (doc, _) = sphere();
        let mut cache = BrickCache::new(CONFIG).expect("cache");
        cache.mark_dirty([-2.0; 3], [2.0; 3]).expect("mark");
        let started = Instant::now();
        let filled = cache
            .refill_all(&doc, use_backend.then_some(&backend), 512)
            .expect("fill");
        println!(
            "  {label:<12} {:>7.2} ms for a whole-model fill of {filled} bricks",
            ms(started.elapsed())
        );
    }
    println!();
}
