//! Where a press goes: the phases `BeginStroke` runs, and what each dirtied.
//!
//! Written for #110. `begin stroke` averages 92.7 ms in the application's own
//! stall ledger while the engine's edit phase reads 2.88 ms, and nothing had
//! reconciled the two. The ViewModel's `BeginStroke` arm does four things —
//! `ensure_tool_available`, `open_live_gesture`, `begin_gesture`, and then
//! `apply_segment`, because "the first dab lands on the press rather than on
//! the first move: a click is a stroke too". So some of what reads as *begin*
//! is a whole stroke application, and this times them apart.
//!
//! It reports COUNTS beside the milliseconds, which is the point: a phase
//! timing alone cannot tell one expensive mesh from six unnecessary ones.
//!
//! One caveat on reading the MESH column. It meshes the keys the press
//! dirtied. When a press dirties nothing — which is what a single-sample Move
//! gesture does, since the drag has not travelled — the empty key list meshes
//! the whole surface instead, so those rows are the cost of a full re-mesh and
//! not the cost of that press.
//!
//! Archived beside the other probes. To run it, put it in
//! `crates/clayspace-engine/tests/` first.
//!
//! ```sh
//! cargo test -p clayspace-engine --release --test begin_stroke_cost -- --nocapture
//! ```
use std::time::Instant;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

fn brush(size: f32) -> BrushSettings {
    BrushSettings { size, intensity: 1.0, ..BrushSettings::default() }
}

fn ms(t: std::time::Duration) -> f64 { t.as_secs_f64() * 1000.0 }

fn press(doc: &mut ClayDocument, tool: ToolKind, at: [f32; 3], sym: [bool; 3], n: usize) {
    // Exactly the phases sculpt_vm's BeginStroke arm runs, in its order.
    let t0 = Instant::now();
    let live = doc.open_live_gesture(tool, sym);
    let t_open = t0.elapsed();

    let t1 = Instant::now();
    SculptModel::begin_gesture(doc);
    let t_begin = t1.elapsed();

    // "The first dab lands on the press rather than on the first move: a
    // click is a stroke too." So BeginStroke ends by applying a segment.
    let samples = vec![GestureSample { position: at, pressure: 1.0, time: 0.0 }];
    let t2 = Instant::now();
    let outcome = doc.apply_stroke(tool, brush(0.35), &samples, sym);
    let t_apply = t2.elapsed();
    let bricks = outcome.map(|o| o.dirty_bricks).unwrap_or(0);

    // What the viewport then has to do with what the edit dirtied: mesh every
    // brick the stroke touched. This is outside apply_stroke and inside the
    // press as a sculptor feels it.
    let keys = doc.take_dirty_keys();
    let t3 = Instant::now();
    let (cache, _) = doc.drawn_cache();
    let meshed = cache
        .mesh(
            Some(doc.document()),
            clayspace_engine::claycore::BrickMeshParams {
                gradient_normals: false,
                colors: false,
                gradient_eps: None,
            },
            &keys,
        )
        .map(|(m, _)| m.indices().len() / 3)
        .unwrap_or(0);
    let t_mesh = t3.elapsed();

    let total = t0.elapsed();
    println!(
        "{n:>3} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>8} {:>8} {meshed:>9}",
        ms(t_open), ms(t_begin), ms(t_apply), ms(total), bricks, keys.len()
    );
    let _ = doc.close_live_gesture();
}

fn run(tool: ToolKind, sym: [bool; 3], label: &str) {
    let Ok(policy) = BackendPolicy::discover(None) else { return };
    let Ok(mut doc) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form) else {
        return;
    };
    println!("\n{label}\n{:>3} {:>9} {:>9} {:>9} {:>9} {:>8} {:>8} {:>9}",
             "#", "open ms", "begin ms", "apply ms", "MESH ms", "bricks", "keys", "tris");
    for n in 1..=6 {
        let a = n as f32 * 0.6;
        press(&mut doc, tool, [0.9 * a.cos(), 0.35 * a.sin(), 0.3], sym, n);
    }
}

#[test]
fn where_does_a_press_go() {
    run(ToolKind::Mover, [false; 3], "Mover, no symmetry");
    run(ToolKind::Mover, [true, false, false], "Mover, x mirror");
    run(ToolKind::Padrao, [true, false, false], "Padrao, x mirror");
}
