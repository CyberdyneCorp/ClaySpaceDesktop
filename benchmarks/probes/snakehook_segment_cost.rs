//! What one `continue stroke` of a Snake Hook actually costs now, in
//! milliseconds, at the segment size the application really sends.
//!
//! The regression tests count bricks, which is the durable thing to assert.
//! This prints time, which is what the sculptor feels, and it walks the pull
//! out to full length so the shape of the curve is visible rather than one
//! number: the whole point of the fix is that these rows should be FLAT, and a
//! row that climbs with the stroke means the quadratic is back.
//!
//! Not a gate. Timing on a shared box is not something to fail a build over.
//!
//! ```sh
//! cargo test -p clayspace-engine --release --test snakehook_segment_cost -- --nocapture
//! ```

use std::time::Instant;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

const ANCHOR: [f32; 3] = [0.0, 0.25, 0.95];
/// `STAMPS_PER_SEGMENT` on a field, which is what the ViewModel waits for.
const PER_SEGMENT: usize = 3;

fn pull(count: usize) -> Vec<GestureSample> {
    (0..count)
        .map(|index| {
            let t = index as f32 / 40.0;
            GestureSample {
                position: [
                    ANCHOR[0] + t * 0.9,
                    ANCHOR[1] + t * 0.55,
                    ANCHOR[2] + t * 0.35,
                ],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.12,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

fn walk(symmetry: [bool; 3], label: &str) {
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };
    SculptModel::begin_gesture(&mut document);
    println!("\n{label}\n{:>9}  {:>10}  {:>8}", "samples", "ms", "bricks");

    let mut sent = PER_SEGMENT;
    while sent <= 40 {
        let path = pull(sent);
        document.take_dirty_keys();
        let started = Instant::now();
        let outcome = document
            .apply_stroke(ToolKind::Puxar, brush(), &path, symmetry)
            .expect("the pull continues");
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        println!("{sent:>9}  {ms:>10.3}  {:>8}", outcome.dirty_bricks);
        sent += PER_SEGMENT;
    }
}

#[test]
fn what_one_segment_costs_as_the_pull_gets_longer() {
    walk([false; 3], "no mirror");
    walk([true, false, false], "mirrored in x");
    println!(
        "\nFlat rows are the fix. A column that climbs with the sample count \
         is the whole-tendril refill having come back."
    );
}
