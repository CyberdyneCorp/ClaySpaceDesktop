//! Where a grab chain starts compounding, in units of its own diameter.
//!
//! `neighbourhood_pricing.rs` shows the two ends: forty-eight disjoint grabs
//! compound not at all, and forty-eight heavily overlapping ones compound
//! hard. This is the middle, which neither side had — the same forty-eight
//! grabs at a sweep of centre-to-centre spacings, reported against the one
//! ratio that should govern it, `d / 2r`.
//!
//! Archived beside the other probes. To run it, put it in
//! `crates/claycore/tests/` first.
//!
//! ```sh
//! cargo test -p claycore --release --test grab_spacing -- --nocapture
//! ```
use claycore::{Document, Item, MoveParams};

/// 48 grabs of fixed radius along a great circle, centre-to-centre `d` apart.
fn declared_at_48(sphere_r: f32, grab_r: f32, d: f32) -> Option<(f32, f32, i32)> {
    let mut doc = Document::new().ok()?;
    let layer = doc.add_sdf_layer("Base").ok()?;
    doc.add_item(layer, &Item::sphere(sphere_r).ok()?).ok()?;
    // Angular step that puts neighbours `d` apart on the surface.
    let step = d / sphere_r;
    for n in 0..48 {
        let a = n as f32 * step;
        let at = [sphere_r * a.cos(), sphere_r * a.sin(), 0.0];
        doc.move_surface(
            layer,
            at,
            [0.02, 0.0, 0.0],
            MoveParams { radius: grab_r, ease: 0, front_only: false },
        )
        .ok()?;
    }
    let r = doc.field_report(layer, 0.5).ok()?;
    Some((r.lipschitz, r.safe_step_scale, r.longest_deformer_chain))
}

#[test]
fn where_compounding_switches_on() {
    const SPHERE: f32 = 5.0;
    const GRAB: f32 = 0.15;
    println!("\n48 grabs, radius {GRAB}, on a radius-{SPHERE} sphere");
    println!("{:>8} {:>9} {:>14} {:>12} {:>7}", "spacing", "d/2r", "declared L", "safe_step", "chain");
    for d in [0.03f32, 0.06, 0.12, 0.18, 0.24, 0.27, 0.30, 0.33, 0.36, 0.45, 0.60] {
        match declared_at_48(SPHERE, GRAB, d) {
            Some((l, s, c)) => println!(
                "{d:>8.2} {:>9.2} {l:>14.3} {s:>12.6} {c:>7}",
                d / (2.0 * GRAB)
            ),
            None => println!("{d:>8.2}  (no backend)"),
        }
    }
}
