//! What a refill may cost the thread that asked for it, and what it may reach.
//!
//! Three defects with one shape between them. An operation marked far more of
//! the field than it had changed, drained all of it before returning, and did
//! so on the interface thread — so the window stopped. The audit measured a
//! curve cancel that held the thread for **over thirty minutes** and never came
//! back, a subtool boolean at 73 seconds that produced nothing, and a copy of a
//! hidden subtool at 66 seconds.
//!
//! So there are two promises here and they are independent:
//!
//! - **What is marked is what changed.** Retiring a sweep dirties the tube and
//!   not the subtool the tube was laid on; a visibility pattern an operation
//!   borrows and puts straight back dirties nothing at all, because the fold
//!   the cache holds is the same fold on both sides of it.
//! - **What is drained is what one budget allows.** The cache keeps whatever a
//!   bounded drain did not take, so the work is stopped rather than dropped and
//!   a later pump finishes it. Whether it was drained in one call or in twenty,
//!   the surface that comes out is the same surface.

use std::collections::HashSet;
use std::time::Duration;

use claycore::BrickKey;
use clayspace_engine::{BackendPolicy, ClayDocument, RefillBudget};
use clayspace_model::{
    BooleanOp, BooleanSettings, Combine, CombineSettings, CurveModel, ObjectModel, SceneModel,
    Shape,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy).expect("a document")
}

/// The unit-radius sphere at the origin every session opens with.
fn with_form() -> ClayDocument {
    ClayDocument::with_starting_form(document()).expect("a document with a starting form")
}

fn adding() -> CombineSettings {
    CombineSettings {
        op: Combine::Add,
        ..CombineSettings::default()
    }
}

/// Where the cache says the surface is, brick by brick.
fn surface(doc: &ClayDocument) -> HashSet<BrickKey> {
    doc.cache()
        .surface_bricks()
        .expect("the cache reports its surface")
        .into_iter()
        .collect()
}

/// How many bricks the cache still has waiting to be filled.
fn waiting(doc: &ClayDocument) -> u64 {
    doc.cache()
        .stats()
        .expect("the cache reports its own state")
        .dirty_bricks
}

/// A curve laid across the front of the form and clear of it, as
/// `tests/curve.rs` lays one.
fn lay(doc: &mut ClayDocument) {
    doc.begin_curve();
    for (at, radius) in [
        ([-0.9f32, 1.4, 0.0], 0.12f32),
        ([0.0, 1.7, 0.0], 0.16),
        ([0.9, 1.4, 0.0], 0.10),
    ] {
        doc.add_curve_point(at, radius).expect("the point");
    }
}

// -- what a retired sweep reaches --------------------------------------------

/// Cancelling a curve dirties the tube, not the subtool it was laid on.
///
/// The regression for the worst figure in the audit. `retire_curve_node` asked
/// for the layer's whole extent **and** for the box the layer vacated, which on
/// a worked subtool are the same thing twice: everything the sculptor has ever
/// put in it. A radius-5 tube made that box 2.3M bricks of a 6.7M budget, and
/// the cancel that was supposed to take the tube back took the session.
///
/// Asked as a set rather than as a count, because the count alone cannot tell a
/// tube's own bricks from the form's — and the form's are exactly what must not
/// be in it. The tube stands clear of the sphere on purpose, so the two sets
/// have an answer.
#[test]
fn retiring_a_curve_dirties_only_its_own_region() {
    let mut doc = with_form();
    let form = surface(&doc);
    assert!(!form.is_empty(), "the starting form built no surface");

    lay(&mut doc);
    doc.take_dirty_keys();

    doc.cancel_curve();

    let retired: HashSet<BrickKey> = doc.take_dirty_keys().into_iter().collect();
    assert!(
        !retired.is_empty(),
        "the cancel dirtied nothing, so the tube is still on the surface"
    );
    let form_again = retired.intersection(&form).count();
    println!(
        "the cancel dirtied {} bricks, {form_again} of which the form's surface \
         occupies out of {}",
        retired.len(),
        form.len()
    );
    // A brick is eight voxels across and the tube's margin rounds into the ones
    // the sphere's north pole reaches, so a handful of shared bricks is the
    // geometry and not the defect. Refilling the layer shares all of them.
    assert!(
        form_again * 4 < form.len(),
        "the cancel re-evaluated {form_again} of the form's {} surface bricks, \
         so it is still refilling the whole subtool the curve was laid on",
        form.len()
    );
}

/// And the tube really is gone: a bound that reached less would be cheaper and
/// wrong, which is the failure this pairs with the one above.
#[test]
fn a_cancelled_curve_leaves_no_tube_behind() {
    let mut doc = with_form();
    let bare = surface(&doc);

    lay(&mut doc);
    assert!(
        surface(&doc).len() > bare.len(),
        "the fixture swept no tube to take back"
    );

    doc.cancel_curve();

    assert_eq!(
        surface(&doc),
        bare,
        "the cancel left bricks holding a sweep that is gone"
    );
}

// -- what a borrowed visibility pattern costs --------------------------------

/// A bake's visibility pattern is borrowed, and a borrowed one costs nothing.
///
/// The mechanism behind the 73-second boolean and the 66-second copy.
/// `with_only_visible` hides every layer but one, samples, and shows them all
/// again — and each of those flags used to mark its layer whole and drain it.
/// Nothing in the field changed: the fold the cache holds is the same fold on
/// both sides of the bracket, brick for brick.
#[test]
fn borrowing_visibility_for_a_bake_refills_nothing() {
    let mut doc = document();
    let kept = doc
        .insert_shape_subtool(Shape::Sphere, &[0.6], [0.0; 3], adding())
        .expect("a sphere subtool")
        .layer;
    for at in 1..4 {
        doc.insert_shape_subtool(Shape::Sphere, &[0.6], [at as f32 * 3.0, 0.0, 0.0], adding())
            .expect("another subtool");
    }
    let before = surface(&doc);
    doc.take_dirty_keys();

    doc.with_only_visible(&[kept], |_| Ok(()))
        .expect("the bracket");

    assert!(
        doc.dirty_keys().is_empty(),
        "borrowing a visibility pattern re-evaluated {} bricks of a field that \
         is the same field on both sides of the bracket",
        doc.dirty_keys().len()
    );
    assert_eq!(waiting(&doc), 0, "the bracket left bricks marked");
    assert_eq!(
        surface(&doc),
        before,
        "the borrowed pattern did not come back"
    );
}

/// A boolean refills where its result stands, not every layer in the document.
///
/// The cost half of the same defect. Each operand is baked with the rest of
/// the scene hidden, so a document of *n* subtools paid roughly 2n whole-layer
/// refills for one boolean — on the audit's document, about 180 of them, which
/// is most of the 73 seconds it took to produce nothing at all.
///
/// The bystanders stand well clear of the pair so that "did the boolean touch
/// them?" is a question the brick keys can answer.
#[test]
fn a_boolean_refills_only_where_its_result_stands() {
    let mut doc = document();
    let sphere = doc
        .insert_shape_subtool(Shape::Sphere, &[0.6], [0.0; 3], adding())
        .expect("a sphere subtool")
        .layer;
    let cylinder = doc
        .insert_shape_subtool(Shape::Cylinder, &[0.25, 1.0], [0.0; 3], adding())
        .expect("a cylinder subtool")
        .layer;
    let mut bystanders: HashSet<BrickKey> = HashSet::new();
    // Emptied first, so what the loop collects is the bystanders' own bricks
    // and not the pair's, which the two inserts above have been accumulating.
    doc.take_dirty_keys();
    for at in 1..4 {
        doc.insert_shape_subtool(Shape::Sphere, &[0.6], [at as f32 * 4.0, 0.0, 0.0], adding())
            .expect("a subtool the boolean never names");
        bystanders.extend(doc.take_dirty_keys());
    }
    // Minus whatever the pair itself reaches, so what is left is the
    // bystanders' alone and a brick the boolean is entitled to is not counted
    // against it.
    for key in surface(&doc) {
        bystanders.remove(&key);
    }
    assert!(!bystanders.is_empty(), "the fixture stood nobody aside");
    doc.take_dirty_keys();

    doc.run_boolean(BooleanSettings {
        base: Some(sphere),
        tool: Some(cylinder),
        op: BooleanOp::Subtract,
        cell_size: 0.02,
        consume: false,
    })
    .expect("a boolean");

    let dirtied: HashSet<BrickKey> = doc.take_dirty_keys().into_iter().collect();
    let touched = dirtied.intersection(&bystanders).count();
    println!(
        "the boolean dirtied {} bricks, {touched} of the {} the bystanders own",
        dirtied.len(),
        bystanders.len()
    );
    assert_eq!(
        touched, 0,
        "the boolean re-evaluated bricks belonging to subtools it never named, \
         so it is still baking by hiding the scene"
    );
    assert_eq!(waiting(&doc), 0, "the boolean left bricks marked");
}

// -- what one drain may spend ------------------------------------------------

/// A bounded drain stops, says so, and keeps what it did not take.
///
/// One brick of budget is not a setting anything ships with; it is the smallest
/// number that makes the boundary fall in the same place on a loaded machine as
/// on an idle one, which is what a test of a budget needs.
#[test]
fn a_bounded_drain_stops_and_says_so() {
    let mut doc = document();
    doc.set_refill_budget(RefillBudget::Bricks(1));

    doc.insert_shape_subtool(Shape::Sphere, &[1.0], [0.0; 3], adding())
        .expect("a sphere subtool");

    assert!(
        doc.refill_is_pending(),
        "a one-brick budget drained a whole sphere without stopping"
    );
    assert!(
        waiting(&doc) > 0,
        "the drain reported work pending and the cache has none"
    );
}

/// A document with no budget set drains in full, which is what every headless
/// caller depends on.
#[test]
fn an_unbudgeted_drain_finishes_before_it_returns() {
    let mut doc = document();

    doc.insert_shape_subtool(Shape::Sphere, &[1.0], [0.0; 3], adding())
        .expect("a sphere subtool");

    assert!(!doc.refill_is_pending(), "the default budget stopped early");
    assert_eq!(waiting(&doc), 0, "the default budget left bricks marked");
}

/// Pumped to the end, a bounded drain reaches the surface a whole one reaches.
///
/// The promise that makes the budget safe to set: it changes *when* the bricks
/// are filled and nothing about what they are filled with. Two documents built
/// by the same calls, one of them a brick at a time.
#[test]
fn pumping_a_bounded_drain_reaches_the_surface_a_whole_one_does() {
    let mut whole = document();
    whole
        .insert_shape_subtool(Shape::Sphere, &[0.8], [0.0; 3], adding())
        .expect("a sphere subtool");

    let mut bounded = document();
    bounded.set_refill_budget(RefillBudget::Bricks(1));
    bounded
        .insert_shape_subtool(Shape::Sphere, &[0.8], [0.0; 3], adding())
        .expect("a sphere subtool");
    assert!(bounded.refill_is_pending(), "the budget did not bind");

    // As the application's frame loop pumps it: one budget a frame, until the
    // cache says there is nothing left. Bounded so a drain that never reports
    // itself finished fails here rather than hanging the suite.
    let mut pumps = 0;
    while bounded.refill_is_pending() {
        bounded.pump_refill().expect("a pump");
        pumps += 1;
        assert!(pumps < 100_000, "the pump never reached the end");
    }
    println!("{pumps} pumps to fill what one whole drain filled");

    assert_eq!(
        surface(&bounded),
        surface(&whole),
        "the surface a pumped refill reached is not the one a whole drain \
         reaches"
    );
}

/// Hiding a worked field subtool is bounded by the budget like anything else.
///
/// The criterion the visibility work handed on. Hiding an SDF layer genuinely
/// changes the field — the cache holds one merged fold and there are no
/// per-layer buffers to drop — so the refill is real and the only way it comes
/// under a frame is by being stopped and continued. What the eye costs is now
/// one budget, and the rest arrives over the frames after it.
#[test]
fn hiding_a_worked_subtool_is_bounded_by_the_budget() {
    let mut whole = document();
    let subtool = whole
        .insert_shape_subtool(Shape::Sphere, &[0.8], [0.0; 3], adding())
        .expect("a subtool worth hiding")
        .layer;
    whole.set_layer_visible(subtool, false).expect("hide it");
    let hidden = surface(&whole);

    let mut bounded = document();
    let subtool = bounded
        .insert_shape_subtool(Shape::Sphere, &[0.8], [0.0; 3], adding())
        .expect("a subtool worth hiding")
        .layer;
    bounded.set_refill_budget(RefillBudget::Bricks(32));
    bounded.take_dirty_keys();

    bounded.set_layer_visible(subtool, false).expect("hide it");

    let cost = bounded.dirty_keys().len();
    println!("the eye cost {cost} bricks before it gave the thread back");
    assert!(
        cost <= 32,
        "hiding a subtool re-evaluated {cost} bricks against a budget of 32, \
         so a visibility write still drains to the end on the caller's thread"
    );
    assert!(
        bounded.refill_is_pending(),
        "the drain stopped and did not say there was more"
    );

    while bounded.refill_is_pending() {
        bounded.pump_refill().expect("a pump");
    }
    assert_eq!(
        surface(&bounded),
        hidden,
        "pumped to the end, the eye did not reach the surface a whole drain \
         reaches"
    );
}

/// Settling finishes what a budget stopped, and leaves the budget alone.
///
/// For the caller that cannot proceed on a surface still catching up. The
/// budget belongs to the host and not to whichever operation last needed an
/// exact answer, so it is still set afterwards.
#[test]
fn settling_finishes_what_a_budget_stopped() {
    let mut doc = document();
    let budget = RefillBudget::Within(Duration::from_nanos(1));
    doc.set_refill_budget(budget);

    doc.insert_shape_subtool(Shape::Sphere, &[1.0], [0.0; 3], adding())
        .expect("a sphere subtool");
    assert!(doc.refill_is_pending(), "the budget did not bind");

    doc.settle_refill().expect("a settle");

    assert!(!doc.refill_is_pending(), "the settle stopped early");
    assert_eq!(waiting(&doc), 0, "the settle left bricks marked");
    assert_eq!(
        doc.refill_budget(),
        budget,
        "the settle kept the budget it borrowed"
    );
}
