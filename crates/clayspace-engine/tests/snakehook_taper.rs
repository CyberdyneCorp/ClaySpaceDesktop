//! What a Snake Hook leaves behind it while the pull is still going.
//!
//! The taper used to be measured across the point *index* — `index / (len -
//! 1)` — so extending a pull renumbered every control point and rewrote every
//! radius with it. Two things came out of that, and only the first is visible
//! to a sculptor: the tendril already drawn kept thickening as they pulled,
//! and, because the field really did change along the whole curve, every
//! segment correctly dirtied every brick the pull had ever reached. Measured
//! on a forty-sample pull that was 880 bricks and 110 ms per segment against
//! the 36 bricks and 0.122 ms the new end needs, and the stroke's cost grew
//! with its own square.
//!
//! Measured with `clay_eval_points` rather than a pick. A pick is answered by
//! a marcher whose own behaviour changes between engine pins, so a difference
//! between two readings through one can be the instrument rather than the
//! field — which is exactly the trap this workspace walked into once already.
//! The field is the thing the taper decides, so the field is what is asserted.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

/// Where the pull starts, on the resting sphere's surface.
const ANCHOR: [f32; 3] = [0.0, 0.25, 0.95];

fn sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// A pull of `count` samples, tracing the same curve however long it is — so
/// the first `n` samples of a long pull are the whole of a short one.
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

/// A document with one pull of `count` samples laid down on it.
fn pulled(count: usize) -> Option<ClayDocument> {
    let mut document = sphere()?;
    document
        .apply_stroke(ToolKind::Puxar, brush(), &pull(count), [false; 3])
        .expect("the starting form takes a pull");
    Some(document)
}

/// Points ringing the path at `index`, close enough to sit inside the tendril
/// and off-axis enough that a change in its *radius* moves them.
fn ring(index: usize) -> Vec<[f32; 3]> {
    let at = pull(index + 1)[index].position;
    let mut points = Vec::new();
    for step in 0..12 {
        let angle = step as f32 / 12.0 * std::f32::consts::TAU;
        for radius in [0.06f32, 0.09, 0.12] {
            points.push([
                at[0] + angle.cos() * radius,
                at[1] + angle.sin() * radius,
                at[2],
            ]);
        }
    }
    points
}

/// The regression.
///
/// A control point already laid down must keep the thickness it was given.
///
/// Probed at index 8 against pulls of twenty and forty samples, which is the
/// pairing that isolates the taper: index 8 is eleven samples — about 0.31,
/// against a reach of roughly 0.18 — back from where the shorter pull stops,
/// so no end cap of either pull is in range and the only thing that can move
/// these probes is the radius the taper gave that point. Under the old
/// index-relative taper it moved: 0.0846 at twenty samples against 0.1027 at
/// forty, clay the sculptor had already put down and was no longer touching.
#[test]
fn a_point_already_pulled_keeps_its_thickness() {
    let Some(short) = pulled(20) else {
        return;
    };
    let Some(long) = pulled(40) else {
        return;
    };

    let probes = ring(8);
    let before = short
        .document()
        .eval_points(None, &probes)
        .expect("the field near an early point");
    let after = long
        .document()
        .eval_points(None, &probes)
        .expect("the field near the same point");

    let worst = before
        .iter()
        .zip(&after)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);

    // The far end of the pull is nowhere near these probes, so extending it
    // may not move them at all. The band is the curve tolerance, not zero:
    // the spline's own fit shifts by a hair when points are appended past it.
    assert!(
        worst < 5e-3,
        "extending the pull moved the field around a point already placed by \
         {worst:.5}; the taper is being measured against the stroke's length \
         again, so clay behind the cursor thickens as the sculptor draws"
    );
}

/// And the pull still tapers — the fix must not have bought stability by
/// flattening the tendril into the tube the taper exists to avoid.
#[test]
fn the_tendril_still_thins_toward_its_tip() {
    let Some(document) = pulled(40) else {
        return;
    };
    let field = |index: usize| -> f32 {
        let probes = ring(index);
        let values = document
            .document()
            .eval_points(None, &probes)
            .expect("the field around the path");
        // The lowest reading is the deepest inside the tendril, so a thicker
        // section reads lower than a thin one at the same offsets.
        values.iter().copied().fold(f32::INFINITY, f32::min)
    };

    let near = field(3);
    let far = field(34);
    assert!(
        near < far,
        "the tendril reads no thicker at its root ({near:.5}) than near its \
         tip ({far:.5}), so the taper has gone"
    );
}

/// The pull is one curve, not a chain of them.
///
/// This is what `replays_from_the_anchor` answers true for, and the reason the
/// whole path is re-sent on every segment. Holding it here so that a change to
/// the dirtying below cannot quietly turn the tendril back into the string of
/// beads that replaying was introduced to fix.
#[test]
fn a_longer_pull_reaches_further_than_a_shorter_one() {
    let Some(short) = pulled(8) else {
        return;
    };
    let Some(long) = pulled(40) else {
        return;
    };
    let tip = ring(36);
    let unreached = short
        .document()
        .eval_points(None, &tip)
        .expect("the field where a short pull did not go");
    let reached = long
        .document()
        .eval_points(None, &tip)
        .expect("the field where a long pull went");

    let inside = |values: &[f32]| values.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(
        inside(&reached) < inside(&unreached),
        "a forty-sample pull put no more material at its far end than an \
         eight-sample one did"
    );
}

/// The second half of the fix: a segment re-evaluates the end it added, not
/// the whole tendril.
///
/// `clay_brick_cache_mark_dirty_nodes` computes a node's own bound, and for a
/// curve that bound is everything the pull has ever reached — so asking it to
/// re-mesh after growing the curve cost every brick under the whole tendril,
/// every segment. Measured through the engine directly, a forty-sample pull
/// dirtied 880 bricks and 110 ms where its new end needs 36 and 0.122.
///
/// Legal only because the taper above is anchored. While a point's radius
/// could still change behind the cursor the whole node genuinely was dirty,
/// and the wide bound was the correct answer to the question being asked.
#[test]
fn growing_a_pull_dirties_the_new_end_and_not_the_whole_tendril() {
    let Some(mut live) = sphere() else {
        return;
    };
    // What the whole tendril costs, as the reference: a fresh document given
    // the same forty samples in one go has to evaluate all of it.
    let Some(mut whole) = pulled(40) else {
        return;
    };
    whole.take_dirty_keys();
    let whole_bricks = whole
        .apply_stroke(ToolKind::Puxar, brush(), &pull(40), [false; 3])
        .map(|outcome| outcome.dirty_bricks)
        .unwrap_or(0);

    SculptModel::begin_gesture(&mut live);
    let authored = live
        .apply_stroke(ToolKind::Puxar, brush(), &pull(20), [false; 3])
        .expect("the pull opens")
        .dirty_bricks;
    // Drained as the viewport drains it every frame. The set is cumulative
    // until something meshes it, so leaving it standing would measure the
    // whole gesture rather than the segment.
    live.take_dirty_keys();
    // The same gesture continuing: the model replays from the anchor, so this
    // is the whole path again with twenty more samples on the end.
    let grown = live
        .apply_stroke(ToolKind::Puxar, brush(), &pull(40), [false; 3])
        .expect("the pull grows")
        .dirty_bricks;

    println!("authored {authored}, grown {grown}, whole {whole_bricks}");
    assert!(
        grown > 1,
        "the grow path reported {grown} dirty bricks; it used to report the \
         constant 1, which is what let this cost hide from the profiler built \
         to find it"
    );
    // Against what the whole tendril costs, which is what a node-wide refill
    // charges — and NOT against `authored`, which also carries the starting
    // form's own bricks and is therefore large enough to hide the defect. That
    // baseline passed with the wide refill still in place.
    assert!(
        grown * 2 < whole_bricks,
        "growing the pull dirtied {grown} bricks where the whole tendril is \
         {whole_bricks}, so the pull is still re-evaluating everything it has \
         ever reached on every segment"
    );
}

/// The mirrored half has to arrive while the pull is being drawn, not when it
/// ends.
///
/// The layer mirror reflects the layer's *items*, so a pull with symmetry on
/// puts a second tendril on the other side of the plane — and dirtying only
/// the box around the newest samples leaves that reflection unmarked. The
/// sculptor then watches one tendril grow live and its mirror image appear all
/// at once when the stroke finishes and something refills the whole layer.
/// Reported from a real session, which is how it was found.
///
/// A count rather than a picture: with the reflection marked, a mirrored
/// segment dirties about twice the bricks an unmirrored one does. With it
/// missed, the two are the same number, which is the defect exactly.
#[test]
fn a_mirrored_pull_dirties_both_halves_while_it_is_drawn() {
    let grown = |symmetry: [bool; 3]| -> Option<usize> {
        let mut document = sphere()?;
        SculptModel::begin_gesture(&mut document);
        document
            .apply_stroke(ToolKind::Puxar, brush(), &pull(20), symmetry)
            .expect("the pull opens");
        // Drained as the viewport drains it, so what follows is the segment's
        // own work rather than the whole gesture's.
        document.take_dirty_keys();
        Some(
            document
                .apply_stroke(ToolKind::Puxar, brush(), &pull(40), symmetry)
                .expect("the pull grows")
                .dirty_bricks,
        )
    };

    let Some(plain) = grown([false; 3]) else {
        return;
    };
    let Some(mirrored) = grown([true, false, false]) else {
        return;
    };

    println!("grown: {plain} plain, {mirrored} mirrored");
    assert!(
        mirrored > plain * 3 / 2,
        "a mirrored segment dirtied {mirrored} bricks against the {plain} an \
         unmirrored one did, so the reflection is not being marked and it will \
         not appear until the stroke ends"
    );
}
