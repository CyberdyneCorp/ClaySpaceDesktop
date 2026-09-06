//! Scratch probe: why does an SDF Snake Hook (`Puxar`) get slower the longer
//! the pull gets?
//!
//! The application replays the WHOLE path from the anchor on every segment —
//! `SculptViewModel::replays_from_the_anchor` answers true for
//! `Representation::Sdf` + `ToolKind::Puxar`, deliberately, so the tendril is
//! one grown curve instead of a string of beads. This measures what that
//! costs, split at the engine boundary:
//!
//!   * `clay_layer_set_stroke_points` — replacing the curve's control points
//!   * `clay_brick_cache_mark_dirty_nodes` — which "computes the bound itself"
//!     (clay.h:9413), and for a curve node that bound is the whole tendril
//!   * the refill that drains it
//!
//! Not for keeping.
//!
//! ```sh
//! cargo test -p clayspace-app --release --test snakehook_scaling -- --nocapture
//! ```

use std::time::{Duration, Instant};

use clayspace_engine::claycore::{
    Backend, BrickCache, BrickConfig, Document, Item, LayerId, NodeId, Op, PointType,
};
use clayspace_engine::BackendPolicy;

const POINT_KIND: PointType = PointType::Spline;
const CURVE_TOLERANCE: f32 = 0.002;
/// The brush radius the taper starts from, as `BrushSettings::default`.
const SIZE: f32 = 0.12;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The path a pull traces: a curving drag across the starting form, exactly
/// the shape the comment in `replays_from_the_anchor` is about.
fn path(points: usize) -> Vec<[f32; 3]> {
    (0..points)
        .map(|i| {
            let t = i as f32 / 40.0;
            [t * 0.9 - 0.45, 0.25 + t * 0.55, 0.85 + t * 0.35]
        })
        .collect()
}

/// The control points the model builds: each sample plus its tapered radius.
fn control(samples: &[[f32; 3]]) -> Vec<f32> {
    let mut out = Vec::with_capacity(samples.len() * 4);
    for (index, at) in samples.iter().enumerate() {
        let t = index as f32 / (samples.len().max(2) - 1) as f32;
        out.extend_from_slice(at);
        out.push(SIZE * (1.0 - 0.7 * t));
    }
    out
}

struct Rig {
    document: Document,
    cache: BrickCache,
    layer: LayerId,
    node: NodeId,
}

impl Rig {
    fn new() -> Option<Self> {
        // `ClayDocument::BRICK_CONFIG`, copied because it is private.
        let config = BrickConfig {
            dim: 8,
            voxel_size: 0.02,
            band_voxels: 3,
            memory_budget: Some(512 * 1024 * 1024),
            colors: false,
        };
        let mut document = Document::new().expect("a document");
        let cache = BrickCache::new(config).expect("a brick cache");
        let layer = match document.layer_ids().expect("layer ids").first() {
            Some(id) => *id,
            None => document.add_sdf_layer("Escultura").expect("a layer"),
        };
        // The starting form, as `ClayDocument::with_starting_form` makes it.
        let body = Item::sphere(1.0).expect("a sphere");
        document.add_item(layer, &body).expect("the starting form");

        // The first segment authors the curve, as `snakehook_stroke` does.
        let points = control(&path(2));
        let mut item = Item::stroke().expect("a stroke item");
        item.set_curve_points(&points, POINT_KIND).expect("points");
        item.set_op(Op::Add).expect("add");
        item.set_stroke_blend_k(SIZE * 0.5).expect("blend");
        let node = document.add_item(layer, &item).expect("the tendril");

        let mut rig = Self {
            document,
            cache,
            layer,
            node,
        };
        rig.settle();
        Some(rig)
    }

    fn settle(&mut self) {
        let _ = self
            .cache
            .mark_dirty_nodes(&self.document, self.layer, &[self.node]);
        while let Ok((requests, _)) = self.cache.take_dirty(512) {
            if requests.is_empty() {
                break;
            }
            let _ = self.cache.refill(&self.document, None, &requests);
        }
    }

    /// One `continue stroke`, split into the three things it does.
    fn grow(&mut self, samples: &[[f32; 3]], backend: Option<&Backend>) -> (f64, f64, f64, usize) {
        let points = control(samples);

        let started = Instant::now();
        self.document
            .set_layer_stroke_points(self.layer, self.node, &points, POINT_KIND, CURVE_TOLERANCE)
            .expect("the curve grows");
        let set = started.elapsed();

        let started = Instant::now();
        let marked = self
            .cache
            .mark_dirty_nodes(&self.document, self.layer, &[self.node])
            .expect("marking");
        let mark = started.elapsed();

        let started = Instant::now();
        let mut refilled = 0usize;
        loop {
            let Ok((requests, _)) = self.cache.take_dirty(512) else {
                break;
            };
            if requests.is_empty() {
                break;
            }
            refilled += requests.len();
            let _ = self.cache.refill(&self.document, backend, &requests);
        }
        let refill = started.elapsed();

        (ms(set), ms(mark), ms(refill), marked.max(refilled))
    }
}

/// What the taper does to the points that are ALREADY down.
///
/// `t` is `index / (len - 1)`, so extending the stroke renumbers every point
/// and every radius with it. This prints the radius at a fixed early index as
/// the pull grows: if it moves, the whole tendril really is changing and the
/// whole-node refill is correct rather than wasteful.
#[test]
fn the_taper_rewrites_the_points_already_down() {
    println!("\n{:>7}  {:>12}  {:>12}", "points", "r[index 2]", "r[index 5]");
    for count in [8usize, 12, 20, 30, 40] {
        let c = control(&path(count));
        println!(
            "{count:>7}  {:>12.5}  {:>12.5}",
            c[2 * 4 + 3],
            c[5 * 4 + 3]
        );
    }
    println!(
        "\nIf these columns move, an existing point's radius changes when the\n\
         pull is extended, so the field along the whole tendril changes too."
    );
}

/// What a fix would buy: refilling only the region the newest samples added,
/// instead of every brick the whole node's bound covers.
#[test]
fn refilling_only_the_new_end_is_what_it_could_cost() {
    let Some(mut rig) = Rig::new() else {
        eprintln!("no engine backend; skipping");
        return;
    };
    let policy = BackendPolicy::discover(None).expect("policy");
    let backend = policy.refill_backend(4096).cloned();

    // Grow to a long pull first, paying the current cost.
    let long = path(40);
    let (_, _, _, whole_bricks) = rig.grow(&long, backend.as_ref());

    // Now dirty ONLY what the last two samples reach, and refill that.
    let tail = &long[long.len() - 2..];
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for at in tail {
        for axis in 0..3 {
            min[axis] = min[axis].min(at[axis] - SIZE);
            max[axis] = max[axis].max(at[axis] + SIZE);
        }
    }
    rig.cache.mark_dirty(min, max).expect("mark the tail");
    let started = Instant::now();
    let mut tail_bricks = 0usize;
    loop {
        let Ok((requests, _)) = rig.cache.take_dirty(512) else {
            break;
        };
        if requests.is_empty() {
            break;
        }
        tail_bricks += requests.len();
        let _ = rig.cache.refill(&rig.document, backend.as_ref(), &requests);
    }
    let tail_ms = ms(started.elapsed());

    println!(
        "\nwhole node : {whole_bricks} bricks\ntail only  : {tail_bricks} bricks, {tail_ms:.3} ms\n\
         ratio      : {:.1}x fewer bricks",
        whole_bricks as f64 / tail_bricks.max(1) as f64
    );
}

#[test]
fn a_pull_costs_more_the_longer_it_gets() {
    let Some(mut rig) = Rig::new() else {
        eprintln!("no engine backend; skipping");
        return;
    };
    let policy = BackendPolicy::discover(None).expect("policy");
    let backend = policy.refill_backend(4096).cloned();

    println!(
        "\n{:>7}  {:>10}  {:>10}  {:>10}  {:>10}  {:>8}",
        "points", "set_pts ms", "mark ms", "refill ms", "total ms", "bricks"
    );
    let mut first = 0.0f64;
    for count in [2usize, 4, 6, 8, 12, 16, 20, 24, 30, 36, 40] {
        let samples = path(count);
        let (set, mark, refill, bricks) = rig.grow(&samples, backend.as_ref());
        let total = set + mark + refill;
        if first == 0.0 {
            first = total.max(0.0001);
        }
        println!(
            "{count:>7}  {set:>10.3}  {mark:>10.3}  {refill:>10.3}  {total:>10.3}  {bricks:>8}  ({:.1}x)",
            total / first
        );
    }
    println!(
        "\nEach row is ONE `continue stroke`. The application sends the whole \n\
         path every time, so a real pull pays the sum of every row above it."
    );
}
