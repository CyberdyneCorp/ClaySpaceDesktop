//! What an edit costs as the layer's history grows: the audit's own series.
//!
//! Issue #174 measured an undo on a flat layer going from 20 ms to 4.5 s over
//! twenty edits while the geometry did not grow, and a Snake Hook segment
//! getting three times dearer over one 60-sample pull. The 200x had two
//! factors multiplied together, and they have different owners:
//!
//! - **How many bricks an undo refills.** A Move segment is a grab at the head
//!   of a node's chain, and `clay_document_undo_bound` used to report the whole
//!   node's bound, dilated by every earlier pull. ClayCore v0.120.1 reports the
//!   head links' balls instead, and `ClayDocument::undo` already refills from
//!   that bound. This is deterministic, so it is asserted exactly.
//! - **What one refilled brick costs.** Every brick evaluates the whole chain,
//!   so this still grows with the edit count. It is the only factor a regional
//!   collapse could still be credited with, so the series records it apart
//!   from the first rather than folding both into one wall time.
//!
//! The wall-time bounds are loose on purpose. A shared runner only ever adds
//! time, so each figure is the fastest of several takes, and the bound is set
//! to catch the 200x class of regression rather than to referee a 2x. The
//! printed series is the measurement; see `clayspace_engine::compaction`.
//!
//! ```sh
//! cargo test -p clayspace-engine --release --test undo_series -- --nocapture
//! ```

use std::sync::Mutex;
use std::time::{Duration, Instant};

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Drag, GestureSample, SceneModel, SculptModel, ToolKind};

/// The audit's series length.
const EDITS: usize = 20;

/// How far the series is carried past the audit's length, so that a cost that
/// grows with the edit count shows as a trend and not as one ratio.
const LONG: usize = 40;

/// The gestures the series is reported at, one-based.
const CHECKPOINTS: [usize; 4] = [1, 10, EDITS, LONG];

/// Takes per figure; the fastest is kept.
const TAKES: usize = 5;

/// The tests in this file time the engine, so they take turns rather than
/// timing each other.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// A Move brush small against the form, so that a grab's reach and its node's
/// bound are far apart and the two ways of bounding an undo can be told apart.
/// Front-only, the setting that grows a chain fastest.
fn small_move_brush() -> BrushSettings {
    BrushSettings {
        size: 0.12,
        intensity: 1.0,
        drag: Drag {
            front_only: true,
            ..Drag::default()
        },
        ..BrushSettings::default()
    }
}

/// One mirrored Move gesture on the same patch, a little way from the last,
/// so every gesture appends two grabs to the starting layer's chain.
fn work_the_patch(document: &mut ClayDocument, index: usize) {
    let at = [0.95, 0.1 + (index as f32 * 0.9).sin() * 0.02, 0.2];
    let samples: Vec<GestureSample> = (0..4)
        .map(|i| GestureSample {
            position: [at[0] + i as f32 * 0.01, at[1], at[2]],
            pressure: 1.0,
            time: i as f32,
        })
        .collect();
    document.begin_gesture();
    document
        .apply_stroke(
            ToolKind::Mover,
            small_move_brush(),
            &samples,
            [true, false, false],
        )
        .expect("a stroke");
    document.end_gesture();
}

fn chain(document: &ClayDocument) -> i32 {
    let key = document.scene().layers[0].key;
    let id = document.layer_id(key).expect("the layer");
    document
        .document()
        .field_report(id, 0.5)
        .expect("a field report")
        .longest_deformer_chain
}

/// One point of the series: undoing the newest gesture.
#[derive(Debug, Clone, Copy)]
struct Undo {
    chain: i32,
    bricks: usize,
    took: Duration,
}

impl Undo {
    fn per_brick(&self) -> f64 {
        self.took.as_secs_f64() / self.bricks.max(1) as f64
    }
}

/// Undoes the gesture just made and puts it back, `TAKES` times, keeping the
/// fastest undo and the bricks it re-meshed.
fn undo_the_newest(document: &mut ClayDocument, entries: usize) -> Undo {
    let mut best = Duration::MAX;
    let mut bricks = 0;
    for _ in 0..TAKES {
        document.take_dirty_keys();
        let started = Instant::now();
        for _ in 0..entries {
            assert!(document.undo().expect("undo"), "nothing to undo");
        }
        best = best.min(started.elapsed());
        bricks = document.take_dirty_keys().len();
        for _ in 0..entries {
            assert!(document.redo().expect("redo"), "nothing to redo");
        }
    }
    Undo {
        chain: chain(document),
        bricks,
        took: best,
    }
}

/// Forty gestures, and the undo of each measured right after it lands.
fn the_series() -> (Vec<Undo>, usize) {
    let mut document = sphere();
    let series = (1..=LONG)
        .map(|index| {
            let depth = document.history().depth;
            work_the_patch(&mut document, index);
            let entries = document.history().depth - depth;
            undo_the_newest(&mut document, entries)
        })
        .collect::<Vec<_>>();
    for (index, undo) in series.iter().enumerate() {
        println!(
            "gesture {:>2}: chain {:>2}, undo {:>7.2} ms, {:>4} bricks, {:>5.1} us a brick",
            index + 1,
            undo.chain,
            undo.took.as_secs_f64() * 1e3,
            undo.bricks,
            undo.per_brick() * 1e6
        );
    }
    (series, document.surface_brick_count())
}

/// The mean of a slice of figures.
fn mean(figures: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = figures.fold((0.0, 0usize), |(sum, count), f| (sum + f, count + 1));
    sum / count.max(1) as f64
}

/// The node-extent factor: the bricks an undo refills do not grow with the
/// chain, and they are the grab's neighbourhood rather than the form.
///
/// Before ClayCore v0.120.1 this was the factor that grew: the undo of a grab
/// refilled its node's whole bound, dilated by every earlier pull. On this
/// fixture that is the sphere, over a thousand surface bricks.
#[test]
fn an_undo_refills_the_grab_and_not_its_node() {
    let _turn = one_at_a_time();
    let (series, surface) = the_series();
    let first = series[0];
    let chain_grew = series[LONG - 1].chain - first.chain;
    assert!(
        chain_grew >= 2 * (LONG as i32 - 1),
        "the fixture must grow a chain to prove anything: grew by {chain_grew}"
    );
    for (index, undo) in series.iter().enumerate() {
        assert!(
            undo.bricks * 4 < surface,
            "gesture {}: an undo re-meshed {} of {surface} surface bricks, the \
             node's bound rather than the grab's",
            index + 1,
            undo.bricks
        );
        assert!(
            undo.bricks * 2 <= first.bricks * 3,
            "gesture {}: an undo re-meshed {} bricks against the first undo's \
             {}; the region grew with the chain",
            index + 1,
            undo.bricks,
            first.bricks
        );
    }
}

/// The acceptance series: after twenty edits an undo costs a small multiple of
/// the first, not 200x. The chain-length factor is reported apart.
///
/// Measured on a Mac (release, v0.120.1): 1.0 ms at the first gesture and 1.4
/// to 3.6 ms at the twentieth, with the bricks flat at 144 and the price of
/// one brick carrying all of the growth. The bound sits well above that and
/// far below the audit's 200x.
#[test]
fn an_undo_after_twenty_edits_costs_near_the_first() {
    let _turn = one_at_a_time();
    let (series, _) = the_series();
    let early = mean(series[..3].iter().map(|u| u.took.as_secs_f64()));
    let late = mean(
        series[EDITS - 3..EDITS]
            .iter()
            .map(|u| u.took.as_secs_f64()),
    );
    let bricks = mean(series[EDITS - 3..EDITS].iter().map(|u| u.bricks as f64))
        / mean(series[..3].iter().map(|u| u.bricks as f64));
    let per_brick = mean(series[EDITS - 3..EDITS].iter().map(Undo::per_brick))
        / mean(series[..3].iter().map(Undo::per_brick));
    let ratio = late / early;
    println!(
        "undo, last three against first three: {ratio:.2}x \
         (bricks {bricks:.2}x, price of a brick {per_brick:.2}x)"
    );
    assert!(
        ratio < 8.0,
        "an undo after {EDITS} edits costs {ratio:.1}x the first (bricks \
         {bricks:.2}x, a brick {per_brick:.2}x): the cost is growing with the \
         edit count again"
    );
}

/// The series carried to forty edits, reported at 1, 10, 20 and 40.
///
/// This is the figure a regional collapse would have to beat: past twenty
/// edits nothing but the chain grows, so whatever the undo gains is the price
/// of one brick over a longer chain. Measured on a Mac (release, v0.120.1,
/// sharing the host with parallel builds): 0.39–0.83 ms at the first gesture
/// and 1.3–3.7 ms at the fortieth, the bricks 126 then flat at 144, and the
/// last three 2.7–4.2x the first three. The bound catches the 200x class, not
/// the chain's own slope.
#[test]
fn an_undo_after_forty_edits_stays_in_its_class() {
    let _turn = one_at_a_time();
    let (series, _) = the_series();
    for at in CHECKPOINTS {
        let undo = series[at - 1];
        println!(
            "checkpoint {at:>2}: chain {:>2}, undo {:.2} ms, {} bricks, {:.1} us a brick",
            undo.chain,
            undo.took.as_secs_f64() * 1e3,
            undo.bricks,
            undo.per_brick() * 1e6
        );
    }
    let early = mean(series[..3].iter().map(|u| u.took.as_secs_f64()));
    let late = mean(series[LONG - 3..].iter().map(|u| u.took.as_secs_f64()));
    let ratio = late / early;
    println!("undo at {LONG} edits against the first three: {ratio:.2}x");
    assert!(
        ratio < 16.0,
        "an undo after {LONG} edits costs {ratio:.1}x the first: the cost is \
         growing with the edit count faster than the chain's own price"
    );
}

/// A Snake Hook pull begun on the worked patch, as `stroke/begin` meets it: the
/// first segment, and the median of the rest. The fastest of `TAKES`, each
/// pull taken back before the next.
fn pull_on_the_patch(document: &mut ClayDocument) -> (f64, f64) {
    let brush = BrushSettings {
        size: 0.18,
        intensity: 0.9,
        ..BrushSettings::default()
    };
    let path: Vec<GestureSample> = (0..30)
        .map(|step| {
            let t = step as f32 / 29.0;
            GestureSample {
                position: [1.0 + 0.4 * t, 0.1 + 0.2 * t, 0.2],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    let (mut begin, mut segment) = (f64::MAX, f64::MAX);
    for _ in 0..TAKES {
        let depth = document.history().depth;
        document.begin_gesture();
        let mut timed = (2..=path.len()).map(|end| {
            let started = Instant::now();
            document
                .apply_stroke(ToolKind::Puxar, brush, &path[..end], [false; 3])
                .expect("the pull was refused");
            started.elapsed().as_secs_f64()
        });
        let first = timed.next().expect("a first segment");
        let mut rest: Vec<f64> = timed.collect();
        document.end_gesture();
        rest.sort_by(f64::total_cmp);
        begin = begin.min(first);
        segment = segment.min(rest[rest.len() / 2]);
        for _ in depth..document.history().depth {
            assert!(document.undo().expect("undo"), "nothing to undo");
        }
    }
    (begin, segment)
}

/// `stroke/begin` and a Snake Hook segment on a layer grown by forty edits.
///
/// The audit measured `stroke/begin` at 4,051 ms on a grown layer (F14). A
/// pull begun on the worked patch is timed at 1, 10, 20 and 40 Move gestures.
/// Measured on a Mac (release, v0.120.1, sharing the host with parallel
/// builds): the first segment 0.68–2.1 ms after one edit and 5.7–7.4 ms after
/// forty, 3.3–8.3x; the median segment 0.85–4.0 ms and 3.8–6.6 ms, 1.7–4.5x.
/// Milliseconds against the audit's four seconds, and all of it the chain's
/// price per brick again. The bound is set against the audit's class, not
/// against that slope, and wide because the first figure is small enough for
/// a loaded host to halve.
#[test]
fn a_stroke_on_a_grown_layer_begins_near_the_first() {
    let _turn = one_at_a_time();
    let mut document = sphere();
    let mut figures = Vec::new();
    for index in 1..=LONG {
        work_the_patch(&mut document, index);
        if CHECKPOINTS.contains(&index) {
            let (begin, segment) = pull_on_the_patch(&mut document);
            println!(
                "after {index:>2} edits: chain {:>2}, begin {:.2} ms, segment {:.2} ms",
                chain(&document),
                begin * 1e3,
                segment * 1e3
            );
            figures.push((begin, segment));
        }
    }
    let (first, last) = (figures[0], figures[figures.len() - 1]);
    let (begin, segment) = (last.0 / first.0, last.1 / first.1);
    println!("after {LONG} edits against one: begin {begin:.2}x, segment {segment:.2}x");
    assert!(
        begin < 16.0 && segment < 16.0,
        "a pull on a layer {LONG} edits deep begins at {begin:.1}x and runs at \
         {segment:.1}x the cost on a fresh one: stroke cost is growing with the \
         edit count again"
    );
}

/// One segment of a pull: what it cost, and the bricks it dirtied.
#[derive(Clone, Copy)]
struct Segment {
    took: f64,
    bricks: usize,
}

/// The audit's cheapest segment, 24 ms. The late-over-early ratio of a pull
/// is refereed only for a late median above it: below, the quotient is of
/// two figures small enough for a loaded host to move on its own.
const AUDIT_EARLY_SEGMENT: f64 = 0.024;

/// The audit's 60-sample Snake Hook pull, drawn away from the sphere.
fn long_pull_path() -> Vec<GestureSample> {
    (0..60)
        .map(|step| {
            let t = step as f32 / 59.0;
            GestureSample {
                position: [1.0 + 0.8 * t, 0.3 * t, 0.1],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// Delivers the pull once, as the interface does: every segment carries the
/// whole path so far, and the dirty set is drained before each one the way
/// the viewport drains it every frame, so a segment's brick count is what that
/// segment dirtied. The pull is undone afterwards so the next take starts
/// where this one did.
fn pull_segments(
    document: &mut ClayDocument,
    brush: BrushSettings,
    path: &[GestureSample],
) -> Vec<Segment> {
    let depth = document.history().depth;
    document.begin_gesture();
    let segments = (1..path.len())
        .map(|end| {
            document.take_dirty_keys();
            let started = Instant::now();
            let outcome = document
                .apply_stroke(ToolKind::Puxar, brush, &path[..=end], [false; 3])
                .expect("the pull was refused");
            Segment {
                took: started.elapsed().as_secs_f64(),
                bricks: outcome.dirty_bricks,
            }
        })
        .collect();
    document.end_gesture();
    for _ in depth..document.history().depth {
        assert!(document.undo().expect("undo"), "nothing to undo");
    }
    segments
}

fn median(values: impl Iterator<Item = f64>) -> f64 {
    let mut sorted: Vec<f64> = values.collect();
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

/// A 60-sample Snake Hook pull, delivered as the interface does, with its
/// late segments held against its early ones: each the median of ten, with
/// the first segment left out because it opens the tendril and warms the
/// cache.
///
/// The audit measured a segment at 24 ms rising to 72 ms over one pull, and
/// before the taper was anchored a pull re-evaluated its whole tendril on
/// every segment — 880 bricks against 150, stalls of 89, 226, 335 and 440 ms
/// (`docs/features.md`). Two things are held, and they have different
/// natures:
///
/// - **The bricks a segment dirties.** The tip's region, not the tendril's:
///   150 early and 125 late on this fixture, the same on every take, machine
///   and backend. This is the deterministic half and the one the quadratic
///   class cannot pass: a segment that re-evaluates the whole curve dirties
///   the union of every tip before it, 421 bricks by the end of this pull.
/// - **What a segment costs**, fastest of five takes. It grows with the path
///   on every machine, because every brick of the tip evaluates the whole
///   tendril, and it grows more the more loaded the host is, because the
///   bricks are evaluated in parallel and a late segment has more of that
///   work to wait for. Measured on an Apple M3 Pro (Metal, v0.126.0, sharing
///   the host with other builds): early 0.46–0.62 ms and late 0.70–2.06 ms,
///   1.3–4.5x over ten release runs, 1.3–2.0x over three debug runs, and
///   6.3x on a warm take with the host loaded. On the hosted macOS runners
///   (`macos-14`, three virtual cores at a load of four to eight per core)
///   it failed in four of thirty jobs between 29 Sep and 7 Oct, in release
///   and in debug, with and without Metal, reading early 0.93–1.80 ms and
///   late 6.82–13.55 ms, 6.5–7.5x, on code the other twenty-six passed (runs
///   36615009192, 36658810228, 37571178170, 37572423835). A ratio of two
///   figures that small is a ratio of the scheduler, so it is refereed only
///   above the audit's own cheapest segment: a late median stays under six
///   times the early one or under 24 ms, whichever is larger. The floor is
///   1.8x the worst runner reading and twelve times this Mac's; the audit's
///   late segment is three times over it and the quadratic class's steps
///   further still. Below it the figures are printed, and `just segments` is
///   where a 2x is refereed. As it stands, fastest of five, this Mac reads
///   0.40–0.47 ms against 0.67–1.60 ms in release (1.5–3.7x) and 0.56–1.34
///   ms against 0.75–2.92 ms in debug (1.4–2.2x), at 150 and 125 bricks
///   every time.
#[test]
fn a_long_pull_keeps_its_segment_cost() {
    let _turn = one_at_a_time();
    let mut document = sphere();
    let brush = BrushSettings {
        size: 0.18,
        intensity: 0.9,
        ..BrushSettings::default()
    };
    let path = long_pull_path();
    let (mut early, mut late) = (f64::MAX, f64::MAX);
    let (mut early_bricks, mut late_bricks) = (0, 0);
    for _ in 0..TAKES {
        let segments = pull_segments(&mut document, brush, &path);
        let (first, last) = (&segments[1..11], &segments[segments.len() - 10..]);
        early = early.min(median(first.iter().map(|s| s.took)));
        late = late.min(median(last.iter().map(|s| s.took)));
        early_bricks = median(first.iter().map(|s| s.bricks as f64)) as usize;
        late_bricks = median(last.iter().map(|s| s.bricks as f64)) as usize;
    }
    let ratio = late / early;
    let bound = (6.0 * early).max(AUDIT_EARLY_SEGMENT);
    println!(
        "pull segment, median of ten, fastest of {TAKES}: early {:.2} ms over \
         {early_bricks} bricks, late {:.2} ms over {late_bricks} bricks, {ratio:.2}x \
         against a bound of {:.1} ms",
        early * 1e3,
        late * 1e3,
        bound * 1e3
    );
    assert!(
        late_bricks * 2 <= early_bricks * 3,
        "a late segment of the pull dirties {late_bricks} bricks against \
         {early_bricks} early: a segment is re-evaluating the tendril again, \
         not its tip"
    );
    assert!(
        late < bound,
        "a pull's late segments cost {:.2} ms, {ratio:.1}x its early ones and over \
         the {:.0} ms the ratio is refereed above: the segment cost is growing \
         with the path again",
        late * 1e3,
        bound * 1e3
    );
}
