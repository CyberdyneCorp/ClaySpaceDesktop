//! Four Move dabs on a plain sphere: what degrades, and per what.
//!
//! Written against the actual reported reproduction, which none of the work in
//! `move-segment-cost.md` had been aimed at: "it takes almost a second after I
//! do 3 or 4 move dabs in a simple sculpture". A simple sculpture is the
//! *low*-density case, so local item density cannot explain it, and every
//! earlier measurement here was taken WITHIN one gesture where this is a
//! question about what accumulates ACROSS them.
//!
//! Each dab is a full gesture — open, three segments, close — and lands
//! somewhere new, which is what a sculptor does and what stops the engine
//! folding one dab into the last. Dabs repeated at the SAME centre and radius
//! coalesce and show none of this; that is worth knowing, and it is why the
//! first version of this probe found nothing.
//!
//! Archived beside the other probes. To run it, put it in
//! `crates/clayspace-engine/tests/` first.
//!
//! ```sh
//! cargo test -p clayspace-engine --release --test move_dab_cost -- --nocapture
//! ```
use std::time::Instant;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SceneModel, SculptModel, ToolKind};

fn brush() -> BrushSettings {
    BrushSettings { size: 0.35, intensity: 1.0, ..BrushSettings::default() }
}

fn dab_path(step: usize, at: [f32; 3]) -> Vec<GestureSample> {
    (0..=step)
        .map(|i| GestureSample {
            position: [at[0] + i as f32 * 0.02, at[1], at[2]],
            pressure: 1.0,
            time: i as f32,
        })
        .collect()
}

fn probe(symmetry: [bool; 3], label: &str) {
    let Ok(policy) = BackendPolicy::discover(None) else { return };
    let Ok(mut doc) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form) else {
        return;
    };
    let key = doc.scene().active_layer().expect("layer").key;
    let id = doc.layer_id(key).expect("id");

    println!("\n{label}\n{:>4}  {:>9}  {:>9}  {:>9}  {:>6}  {:>12}",
             "dab", "total ms", "close ms", "rays ms", "chain", "step_scale");
    for dab in 1..=8 {
        // Each dab somewhere new, which is what a sculptor does — and what
        // stops the engine folding them into one another.
        let a = dab as f32 * 0.7;
        let at = [0.30 * a.cos(), 0.30 * a.sin(), 0.92];
        let started = Instant::now();
        SculptModel::begin_gesture(&mut doc);
        doc.open_live_gesture(ToolKind::Mover, symmetry);
        for step in 1..=3 {
            doc.apply_stroke(ToolKind::Mover, brush(), &dab_path(step, at), symmetry)
                .expect("segment");
        }
        let close_started = Instant::now();
        doc.close_live_gesture().expect("close");
        let close_ms = close_started.elapsed().as_secs_f64() * 1000.0;
        let total_ms = started.elapsed().as_secs_f64() * 1000.0;

        // What a pick costs: sphere-tracing the field, which is what a
        // collapsing safe step scale makes expensive. 256 rays, the shape of
        // one pointer-down plus a little of what a render does.
        let rays_started = Instant::now();
        for i in 0..256 {
            let a = i as f32 * 0.0245;
            let origin = [3.0 * a.cos(), 3.0 * a.sin(), 1.5];
            let dir = [-a.cos(), -a.sin(), -0.5];
            let _ = doc.document().raycast(origin, dir);
        }
        let mesh_ms = rays_started.elapsed().as_secs_f64() * 1000.0;

        let report = doc.document().field_report(id, 0.0).expect("report");
        let cost = doc.layer_cost(key).expect("cost");
        println!("{dab:>4}  {total_ms:>9.2}  {close_ms:>9.2}  {mesh_ms:>9.2}  {:>6}  {:>12.6}  advise={}",
                 report.longest_deformer_chain, cost.safe_step_scale,
                 report.advises_consolidation);
    }
}

#[test]
fn four_move_dabs_on_a_simple_sphere() {
    probe([false; 3], "no symmetry");
    probe([true, false, false], "x mirror");
}
