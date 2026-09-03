//! The queue a host drains between interactions, at the engine boundary.
//!
//! Two claims, and both are mechanisms rather than conventions, so both are
//! measured here rather than read:
//!
//! - **The gate refuses.** `nothing_may_be_taken_or_completed_while_a_stroke_-
//!   is_open` is the whole reason the queue holds a stroke flag at all: a host
//!   that wired its drain to the wrong callback finds out by nothing happening
//!   rather than by a stutter it will blame on the brush.
//! - **Take and complete are two halves.** `an_item_taken_and_not_completed_is_-
//!   still_there` is a host declining work it could not afford, which is the
//!   difference between a budget and a drop.
//!
//! Nothing here performs any maintenance: an item is a request naming a kind
//! and a target, and what services it is an ordinary entry point the host
//! already has. That is what makes every one of these tests a statement about
//! bookkeeping and none of them a statement about a surface.

use claycore::{MaintenanceKind, MaintenanceQueue};

// -- the fold ---------------------------------------------------------------

/// A stroke asks for the same rebuild on every dab. Folding them into one
/// entry is what makes the request safe to call from a stamp.
#[test]
fn the_same_request_folds_into_one_entry_and_counts_the_asking() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    assert!(queue.is_empty().expect("count"));

    for _ in 0..12 {
        queue
            .request(MaintenanceKind::IndexRebuild, 7, 0)
            .expect("request");
    }

    assert_eq!(
        queue.len().expect("count"),
        1,
        "twelve dabs asking for the same rebuild made twelve entries"
    );
    let item = queue.item(0).expect("item");
    assert_eq!(item.kind, MaintenanceKind::IndexRebuild);
    assert_eq!(item.target, 7);
    assert_eq!(
        item.requests, 12,
        "the fold has to count the asking, because an entry whose count keeps \
         climbing is one the host is starving"
    );
}

/// The target is what makes two requests the same request. It is never
/// interpreted, so two levels are two jobs.
#[test]
fn the_same_kind_at_a_different_target_is_a_different_job() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    queue
        .request(MaintenanceKind::ChunkCompaction, 1, 0)
        .expect("request");
    queue
        .request(MaintenanceKind::ChunkCompaction, 2, 0)
        .expect("request");

    assert_eq!(queue.len().expect("count"), 2);
    assert!(queue.has(MaintenanceKind::ChunkCompaction, 1).expect("has"));
    assert!(queue.has(MaintenanceKind::ChunkCompaction, 2).expect("has"));
    assert!(
        !queue.has(MaintenanceKind::ChunkCompaction, 3).expect("has"),
        "a target nobody asked for is queued"
    );
    assert!(
        !queue.has(MaintenanceKind::NormalFlush, 1).expect("has"),
        "a kind nobody asked for is queued at a target that was"
    );
}

/// A host that has measured its own device replaces the estimate by asking
/// again with a figure of its own; zero stays "unknown" rather than
/// overwriting one.
#[test]
fn the_latest_non_zero_estimate_wins_and_zero_does_not_erase_one() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    queue
        .request(MaintenanceKind::DetailPromotion, 0, 0)
        .expect("unknown");
    assert_eq!(queue.item(0).expect("item").estimated_micros, 0);

    queue
        .request(MaintenanceKind::DetailPromotion, 0, 2_500)
        .expect("measured");
    assert_eq!(queue.item(0).expect("item").estimated_micros, 2_500);

    queue
        .request(MaintenanceKind::DetailPromotion, 0, 0)
        .expect("unknown again");
    assert_eq!(
        queue.item(0).expect("item").estimated_micros,
        2_500,
        "a caller with no figure erased one that had been measured"
    );
}

// -- the gate ---------------------------------------------------------------

/// The mechanism. Not "we only call this between strokes" — the queue refuses,
/// so a drain wired to the wrong callback does nothing rather than stuttering.
#[test]
fn nothing_may_be_taken_or_completed_while_a_stroke_is_open() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    queue
        .request(MaintenanceKind::NormalFlush, 4, 0)
        .expect("request");
    assert!(!queue.in_stroke().expect("in stroke"));

    {
        let mut stroke = queue.stroke().expect("open a stroke");
        assert!(stroke.in_stroke().expect("in stroke"));

        assert!(
            stroke.take_next().expect("take").is_none(),
            "the queue handed out work with a finger on the glass"
        );
        assert!(
            !stroke
                .complete(MaintenanceKind::NormalFlush, 4)
                .expect("complete"),
            "an item completed mid-stroke would have been performed mid-stroke"
        );

        // Requesting is exactly what a stamp does, and it is not gated.
        stroke
            .request(MaintenanceKind::IndexRebuild, 4, 0)
            .expect("a stamp asks");
        assert_eq!(stroke.len().expect("count"), 2);
    }

    assert!(
        !queue.in_stroke().expect("in stroke"),
        "the guard went out of scope and the stroke is still open"
    );
    assert!(
        queue.take_next().expect("take").is_some(),
        "the stroke ended and the queue is still refusing"
    );
    assert_eq!(
        queue.len().expect("count"),
        2,
        "the gate dropped what was requested during the stroke"
    );
}

/// The guard's whole reason for existing: a stroke loop that leaves early
/// must not leave the queue shut forever, because the symptom is maintenance
/// that silently never runs again — the failure the gate exists to make loud,
/// turned back into a silent one.
#[test]
fn a_stroke_that_returns_early_still_closes() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    queue
        .request(MaintenanceKind::IndexRebuild, 0, 0)
        .expect("request");

    let refused: claycore::Result<()> = (|| {
        let mut stroke = queue.stroke()?;
        stroke.request(MaintenanceKind::NormalFlush, 0, 0)?;
        // The `?` every stroke loop has. Here it is a refused request, which
        // is the one failure this queue can actually produce; in a host it is
        // a refused tool or a segment the engine would not take.
        stroke.request(MaintenanceKind::Unknown(9_999), 0, 0)?;
        unreachable!("the engine accepted a kind it does not declare");
    })();
    assert!(refused.is_err(), "the fixture did not leave early");

    assert!(
        !queue.in_stroke().expect("in stroke"),
        "a stroke left open by an early return shuts the queue forever"
    );
    assert!(
        queue.take_next().expect("take").is_some(),
        "the queue is still refusing work after the stroke unwound"
    );
}

/// The same claim against an unwind, which is the case a hand-written
/// `end_stroke` at the bottom of a function does not cover.
#[test]
fn a_stroke_that_panics_still_closes() {
    let mut queue = MaintenanceQueue::new().expect("queue");

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _stroke = queue.stroke().expect("open a stroke");
        panic!("a stamp went wrong with a finger on the glass");
    }));
    assert!(panicked.is_err(), "the fixture did not panic");

    assert!(
        !queue.in_stroke().expect("in stroke"),
        "the queue is shut for the life of the process after one panicking \
         stroke"
    );
}

// -- take and complete ------------------------------------------------------

/// `take_next` peeks. A host that took an item and then found it could not
/// afford it has declined rather than dropped it.
#[test]
fn an_item_taken_and_not_completed_is_still_there() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    queue
        .request(MaintenanceKind::NormalFlush, 3, 0)
        .expect("request");

    let first = queue.take_next().expect("take").expect("an item");
    assert_eq!(first.kind, MaintenanceKind::NormalFlush);
    assert_eq!(
        queue.len().expect("count"),
        1,
        "taking an item removed it, so a host that declined it has lost it"
    );

    let again = queue.take_next().expect("take").expect("an item");
    assert_eq!(
        (again.kind, again.target),
        (first.kind, first.target),
        "the declined item was not offered again"
    );

    assert!(
        queue
            .complete(MaintenanceKind::NormalFlush, 3)
            .expect("complete"),
        "completing the item the queue just handed out reported nothing there"
    );
    assert!(queue.is_empty().expect("count"));
    assert!(
        queue.take_next().expect("take").is_none(),
        "an empty queue handed out work"
    );
}

#[test]
fn completing_something_that_was_never_queued_says_so() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    assert!(
        !queue
            .complete(MaintenanceKind::SlotPoolCompaction, 1)
            .expect("complete"),
        "the queue reported having removed an entry it never held"
    );
}

/// The budget loop the header spells out, in the caller's own language,
/// because the caller is the one holding the clock the budget is measured on.
#[test]
fn a_budgeted_drain_takes_what_it_can_afford_and_leaves_the_rest() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    for target in 0..5u32 {
        queue
            .request(MaintenanceKind::ChunkCompaction, target, 1_000)
            .expect("request");
    }

    let mut budget_micros = 2_500u64;
    let mut done = Vec::new();
    while let Some(item) = queue.take_next().expect("take") {
        if item.estimated_micros > budget_micros {
            // Declined, not dropped — and the loop has to stop, because the
            // same item is what comes back next.
            break;
        }
        budget_micros -= item.estimated_micros;
        queue.complete(item.kind, item.target).expect("complete");
        done.push(item.target);
    }

    assert_eq!(
        done.len(),
        2,
        "a 2500 µs budget did {} × 1000 µs",
        done.len()
    );
    assert_eq!(
        queue.len().expect("count"),
        3,
        "the unaffordable items were dropped rather than left waiting"
    );
    for target in &done {
        assert!(
            !queue
                .has(MaintenanceKind::ChunkCompaction, *target)
                .expect("has"),
            "target {target} was completed and is still queued"
        );
    }
}

#[test]
fn clearing_the_queue_throws_the_work_away() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    for kind in MaintenanceKind::ALL {
        queue.request(kind, 0, 0).expect("request");
    }
    assert_eq!(queue.len().expect("count"), MaintenanceKind::ALL.len());
    assert_eq!(
        queue.items().expect("items").len(),
        MaintenanceKind::ALL.len()
    );

    // Correct at any moment, because none of what is queued was correctness.
    queue.clear().expect("clear");
    assert!(queue.is_empty().expect("count"));
}

// -- the vocabulary ---------------------------------------------------------

#[test]
fn every_kind_the_engine_declares_is_expressible_and_named() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    for kind in MaintenanceKind::ALL {
        queue
            .request(kind, 0, 0)
            .unwrap_or_else(|e| panic!("the engine refused {kind:?}: {e}"));
        assert!(!kind.text().is_empty(), "{kind:?} has no name");
    }

    let mut names: Vec<&str> = MaintenanceKind::ALL.iter().map(|k| k.text()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names.len(),
        MaintenanceKind::ALL.len(),
        "two kinds share a name, so a host's diagnostics cannot tell them apart"
    );
}

/// A kind outside the engine's list is a refusal rather than a clamp: mapping
/// it onto the default would queue an index rebuild for a caller that asked
/// for something else, and the host would service it without ever learning it
/// had been misheard.
#[test]
fn a_kind_this_build_does_not_know_is_refused_rather_than_clamped() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    let stranger = MaintenanceKind::Unknown(9_999);

    assert!(
        queue.request(stranger, 0, 0).is_err(),
        "an unknown kind was accepted, and whatever it was queued as is not \
         what the caller asked for"
    );
    assert!(queue.is_empty().expect("count"));
    assert!(
        !stranger.text().is_empty(),
        "the engine names every value, including one it does not know"
    );
}

#[test]
fn the_queue_says_what_it_costs() {
    let mut queue = MaintenanceQueue::new().expect("queue");
    let empty = queue.bytes().expect("bytes");
    for target in 0..8u32 {
        queue
            .request(MaintenanceKind::IndexRebuild, target, 0)
            .expect("request");
    }
    assert!(
        queue.bytes().expect("bytes") >= empty,
        "eight entries cost less than none"
    );
}

// -- the gate across a gesture that is not a block --------------------------

/// An interactive host's stroke is a field, not a scope: it opens on a press
/// and closes on a release that arrives as a separate event. The owned form is
/// the same gate for that shape.
#[test]
fn a_gesture_held_across_frames_gates_and_hands_the_queue_back() {
    let queue = MaintenanceQueue::new().expect("queue");
    let mut gesture = queue.into_stroke().expect("open a gesture");

    assert!(gesture.in_stroke().expect("in stroke"));
    for _ in 0..8 {
        // Every dab of the drag, folded into one entry — the call the gate is
        // built to keep cheap.
        gesture
            .request(MaintenanceKind::IndexRebuild, 4, 0)
            .expect("request");
    }
    assert_eq!(gesture.len().expect("count"), 1);
    assert!(
        gesture.take_next().expect("take").is_none(),
        "the gate handed out work with a finger on the glass"
    );

    let mut queue = gesture.end();
    assert!(!queue.in_stroke().expect("in stroke"));
    let item = queue
        .take_next()
        .expect("take")
        .expect("the drag's request should be there once the gesture ended");
    assert_eq!(item.kind, MaintenanceKind::IndexRebuild);
    assert_eq!(item.requests, 8, "the fold lost count of the asking");
}

/// The shape a host actually holds: the gesture lives in a field, so an
/// unwind past the frame that opened it does not end it — and the queue is
/// still inside it, still gated, still drainable the moment the gesture does
/// end. The `Drop` half is what makes "still inside it" safe rather than a
/// leak, and it is observable one layer up, where the field goes away with the
/// document that held it.
#[test]
fn a_gesture_held_in_a_field_survives_an_unwind_and_ends_drainable() {
    let mut held = Some(
        MaintenanceQueue::new()
            .expect("queue")
            .into_stroke()
            .expect("open a gesture"),
    );

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let gesture = held.as_mut().expect("a gesture is open");
        gesture
            .request(MaintenanceKind::NormalFlush, 1, 0)
            .expect("request");
        panic!("a stamp went wrong with a finger on the glass");
    }));
    assert!(panicked.is_err(), "the fixture did not panic");

    let gesture = held.expect("the gesture outlived the unwind");
    assert!(
        gesture.in_stroke().expect("in stroke"),
        "an unwind past a held gesture is not a pointer release"
    );

    let mut queue = gesture.end();
    assert!(!queue.in_stroke().expect("in stroke"));
    assert!(
        queue.take_next().expect("take").is_some(),
        "what the gesture recorded did not survive it"
    );
}
