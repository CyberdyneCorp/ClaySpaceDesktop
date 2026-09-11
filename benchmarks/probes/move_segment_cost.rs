//! What one segment of a live Move drag costs, and where the cost is.
//!
//! Written to settle a report — "the performance of the Move brushes in SDF is
//! not realtime" — against two competing explanations, only one of which
//! survives measurement.
//!
//! The engine side priced the other one first and it lost. Drawing each
//! resolved grab onto the layer and undoing it inside the same segment, which
//! is what [`clayspace_engine`]'s `LiveMove::settle` does, costs **1.02x** the
//! `clay_sdf_move_preview_document` route that exists to replace it — measured
//! in C, 240 samples per arm, with the draw-then-undo arm handed the
//! advantages. Two document mutations and an undo round-trip are noise beside
//! the refill they sit next to. So the pattern is not the cost and neither is
//! warp accumulation: the chain is one grab per gesture on this host, which
//! `live_transactions.rs` already asserts at 12 against a segmented 72.
//!
//! That leaves the refill, and one property of this host's in particular.
//! `refill_preview` re-fills the **union** of where the last preview stood and
//! where this one does. For a drag that grows away from its anchor both boxes
//! grow, so the union grows with the length of the drag rather than with the
//! distance the pointer moved since the last event. If that is what is
//! happening, the rows below climb, and a climbing row is a cost the sculptor
//! feels as the stroke getting heavier the longer they pull.
//!
//! This is the same shape as the Snake Hook quadratic this repository already
//! found and fixed once — see `snakehook_segment_cost.rs`, whose rows are flat
//! because of that fix. Flat rows here mean the union is not the problem and
//! the cost is simply the swept ball's brick count, which is a different
//! conversation (fewer bricks, not fewer refills).
//!
//! Not a gate. Timing on a shared box is not something to fail a build over.
//!
//! Archived here rather than in the crate, like the probes beside it. To run
//! it, put it in `crates/clayspace-engine/tests/` first — a probe is an
//! instrument for one question, not a suite member that has to keep compiling.
//!
//! ```sh
//! cargo test -p clayspace-engine --release --test move_segment_cost -- --nocapture
//! ```

use std::time::Instant;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

/// On the starting form's surface, so the drag has material to take hold of.
const ANCHOR: [f32; 3] = [0.0, 0.0, 0.95];

/// The pointer path: away from the anchor, so the drag grows.
///
/// `samples[0]` is the press and stays the press — that is what anchors the
/// transaction, and sending a growing slice of this is what the ViewModel does
/// per pointer event.
fn drag(count: usize) -> Vec<GestureSample> {
    (0..count)
        .map(|index| {
            let t = index as f32 / 40.0;
            GestureSample {
                position: [ANCHOR[0] + t * 0.7, ANCHOR[1] + t * 0.4, ANCHOR[2]],
                pressure: 1.0,
                time: index as f32,
            }
        })
        .collect()
}

fn brush(size: f32) -> BrushSettings {
    BrushSettings {
        size,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// Stamps a blockout onto the starting form, so the drag has a scene to pay
/// for rather than a bare sphere.
///
/// The engine-side probe ran against 200 stamps and measured 14.2 ms per
/// pointer event; a starting form on its own measures under 2. Whatever
/// "not realtime" is, it is not visible on an empty scene, so the scene is a
/// variable of the measurement and not a backdrop to it.
fn blockout(document: &mut ClayDocument, stamps: usize) {
    for index in 0..stamps {
        // Scattered over the form on a spiral, so the stamps land apart
        // instead of stacking into one lump the field can collapse.
        let t = index as f32 / stamps.max(1) as f32;
        let angle = t * std::f32::consts::TAU * 7.0;
        let height = -0.8 + 1.6 * t;
        let ring = (1.0f32 - height * height).max(0.0).sqrt() * 0.95;
        let at = [ring * angle.cos(), height, ring * angle.sin()];
        let dab = [
            GestureSample {
                position: at,
                pressure: 1.0,
                time: 0.0,
            },
            GestureSample {
                position: at,
                pressure: 1.0,
                time: 1.0,
            },
        ];
        let _ = document.apply_stroke(ToolKind::Padrao, brush(0.12), &dab, [false; 3]);
    }
}

/// One drag, timed per pointer event, at one brush size and one scene load.
fn walk(size: f32, stamps: usize) {
    let Ok(policy) = BackendPolicy::discover(None) else {
        println!("no backend; skipping");
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        println!("no engine; skipping");
        return;
    };

    blockout(&mut document, stamps);

    SculptModel::begin_gesture(&mut document);
    // The live path is the one under test. If this is false the drag would
    // fall back to `move_surface_stroke`, which is a different path with a
    // different cost, and the numbers below would be answering another
    // question than the one asked.
    if !document.open_live_gesture(ToolKind::Mover, [false; 3]) {
        println!("live Move did not arm — active layer is not an editable SDF");
        return;
    }

    let path = drag(40);
    println!(
        "\nbrush size {size}, {stamps} stamps\n{:>7}  {:>10}  {:>8}",
        "event", "ms", "bricks"
    );
    let mut total = 0.0;
    for sent in 1..path.len() {
        document.take_dirty_keys();
        let started = Instant::now();
        let outcome = document
            .apply_stroke(ToolKind::Mover, brush(size), &path[..=sent], [false; 3])
            .expect("the drag continues");
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        total += ms;
        // Every fourth, so the shape is visible without forty rows per size.
        if sent % 4 == 0 {
            println!("{sent:>7}  {ms:>10.3}  {:>8}", outcome.dirty_bricks);
        }
    }
    let events = path.len() - 1;
    println!(
        "  mean over {events} events: {:.3} ms",
        total / events as f64
    );
    let _ = document.close_live_gesture();
}

#[test]
fn what_one_segment_of_a_move_drag_costs_as_the_drag_lengthens() {
    // Radius first, on an empty scene: what the swept ball costs by itself.
    for size in [0.10f32, 0.20, 0.40] {
        walk(size, 0);
    }
    // Then scene load at one radius: what the same drag costs as the form
    // under it fills up. This is the axis the engine-side probe varied and
    // this one had not.
    for stamps in [50usize, 200, 400] {
        walk(0.40, stamps);
    }
    println!(
        "\nTwo axes. Within a block, a climbing row is the drag getting heavier \
         the longer it is pulled. Across blocks, a rising mean is the scene \
         being paid for on every pointer event."
    );
}
