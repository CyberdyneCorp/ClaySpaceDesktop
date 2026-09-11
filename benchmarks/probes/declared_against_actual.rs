//! The Lipschitz bound a deformed layer DECLARES against the one it has.
//!
//! A grab chain's safe step scale collapses geometrically — 0.89 at one
//! gesture, 0.0043 at forty-eight — and the marcher takes its step count from
//! that. The question that decides whether a sampled warp volume is worth
//! building is whether the collapse reflects the field or only the bookkeeping:
//! the declared bound is the PRODUCT of each deformer's declared factor, and a
//! product of conservative bounds is conservative to the power of the chain.
//!
//! So this measures both. `clay_layer_field_report.lipschitz` is what the layer
//! declares; the actual gradient is central differences over a shell of points
//! straddling the worked surface, which is the local Lipschitz a ray would
//! really meet.
//!
//! **The actual gradient is a sampled maximum over 600 points and is therefore
//! a LOWER bound on the true one.** A worse point may exist between samples.
//! The slack column is accordingly an upper estimate — but at 99.9x it would
//! take a two-order-of-magnitude sampling miss to change the conclusion.
//!
//! Archived beside the other probes. To run it, put it in
//! `crates/claycore/tests/` first.
//!
//! ```sh
//! cargo test -p claycore --release --test declared_against_actual -- --nocapture
//! ```
use claycore::{Document, Item, LayerId, MoveParams};

fn sphere() -> (Document, LayerId) {
    let mut doc = Document::new().expect("doc");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.add_item(layer, &Item::sphere(1.0).expect("s")).expect("place");
    (doc, layer)
}

/// Max |grad f| by central differences over a shell of points around the
/// worked patch — the ACTUAL local Lipschitz the marcher would meet.
fn actual_gradient(doc: &Document, h: f32) -> f32 {
    let mut worst = 0.0f32;
    for i in 0..600 {
        let a = i as f32 * 0.7919;
        let b = i as f32 * 0.3313;
        // A shell straddling the surface near the patch.
        let r = 0.88 + 0.24 * ((i % 11) as f32 / 10.0);
        let p = [
            r * a.cos() * b.sin(),
            r * a.sin() * b.sin(),
            r * b.cos(),
        ];
        let mut g = [0.0f32; 3];
        let mut ok = true;
        for axis in 0..3 {
            let mut lo = p; lo[axis] -= h;
            let mut hi = p; hi[axis] += h;
            match doc.eval_points(None, &[lo, hi]) {
                Ok(v) if v.len() == 2 => g[axis] = (v[1] - v[0]) / (2.0 * h),
                _ => { ok = false; break; }
            }
        }
        if ok {
            let m = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
            if m.is_finite() && m > worst { worst = m; }
        }
    }
    worst
}

#[test]
fn declared_against_actual_as_the_chain_deepens() {
    let (mut doc, layer) = sphere();
    println!("\n{:>5} {:>8} {:>12} {:>12} {:>10} {:>9}",
             "moves", "chain", "declared L", "safe_step", "actual |g|", "slack");
    for n in 1..=48 {
        let a = n as f32 * 0.41;
        let at = [0.92 * a.cos(), 0.30 * a.sin(), 0.25];
        doc.move_surface(layer, at, [0.035, 0.02, 0.0],
            MoveParams { radius: 0.42, ease: 0, front_only: false })
            .expect("a gesture");
        if !matches!(n, 1 | 4 | 8 | 16 | 32 | 48) { continue; }
        let r = doc.field_report(layer, 0.5).expect("report");
        let actual = actual_gradient(&doc, 0.004);
        println!("{n:>5} {:>8} {:>12.2} {:>12.6} {:>10.3} {:>8.1}x",
                 r.longest_deformer_chain, r.lipschitz, r.safe_step_scale,
                 actual, if actual > 0.0 { r.lipschitz / actual } else { 0.0 });
    }
}
