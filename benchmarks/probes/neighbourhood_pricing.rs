//! Whether the per-link neighbourhood pricing actually bites.
//!
//! `deformer_lipschitz` does not naively multiply every link: ClayCore #386
//! replaced a running product with connected components and #452 replaced that
//! with per-link neighbourhood pricing, which multiplies only links that can
//! genuinely meet at a point. The question this answers is whether that work
//! reaches a real chain, because if it does not, the slack measured in
//! `declared_against_actual.rs` has a much cheaper explanation than
//! submultiplicativity.
//!
//! It does. Forty-eight disjoint grabs compound not at all.
//!
//! Archived beside the other probes. To run it, put it in
//! `crates/claycore/tests/` first.
//!
//! ```sh
//! cargo test -p claycore --release --test neighbourhood_pricing -- --nocapture
//! ```
use claycore::{Document, Item, LayerId, MoveParams};

fn big_sphere(r: f32) -> (Document, LayerId) {
    let mut doc = Document::new().expect("doc");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.add_item(layer, &Item::sphere(r).expect("s")).expect("place");
    (doc, layer)
}

/// Fibonacci sphere: as evenly spread as 48 points get.
fn spread(n: usize, count: usize, r: f32) -> [f32; 3] {
    let ga = std::f32::consts::PI * (3.0 - 5.0f32.sqrt());
    let y = 1.0 - (n as f32 / (count - 1) as f32) * 2.0;
    let rad = (1.0 - y * y).max(0.0).sqrt();
    let th = ga * n as f32;
    [r * th.cos() * rad, r * y, r * th.sin() * rad]
}

fn run(label: &str, sphere_r: f32, grab_r: f32, packed: bool) {
    let (mut doc, layer) = big_sphere(sphere_r);
    println!("\n{label}");
    println!("{:>5} {:>8} {:>14} {:>12}", "moves", "chain", "declared L", "safe_step");
    for n in 0..48 {
        let at = if packed {
            let a = n as f32 * 0.41;
            [0.92 * a.cos(), 0.30 * a.sin(), 0.25]
        } else {
            spread(n, 48, sphere_r)
        };
        doc.move_surface(layer, at, [0.02, 0.01, 0.0],
            MoveParams { radius: grab_r, ease: 0, front_only: false })
            .expect("a gesture");
        let m = n + 1;
        if !matches!(m, 1 | 8 | 24 | 48) { continue; }
        let r = doc.field_report(layer, 0.5).expect("report");
        println!("{m:>5} {:>8} {:>14.2} {:>12.6}",
                 r.longest_deformer_chain, r.lipschitz, r.safe_step_scale);
    }
}

#[test]
fn does_the_neighbourhood_pricing_bite() {
    // Mine, as measured before: grabs that revisit one region.
    run("PACKED: 0.42 grabs revisiting one band", 1.0, 0.42, true);
    // Spread over a large sphere so no two grabs can meet at a point.
    run("SPREAD: 0.15 grabs on a radius-3 sphere", 3.0, 0.15, false);
}
