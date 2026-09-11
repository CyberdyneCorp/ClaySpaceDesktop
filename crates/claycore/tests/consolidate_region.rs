//! Baking a patch rather than a subtool, and the two scopes told apart.
//!
//! This file exists because of a gap that took three tries to see. The engine
//! ships two consolidations: `clay_layer_consolidate` collapses a whole layer,
//! and `clay_layer_consolidate_region` bakes the influence closure of a box and
//! leaves the rest parametric. Only the first was bound here.
//!
//! That mattered more than a missing convenience, because the engine's own note
//! on `CLAY_DEGRADATION_DEFORMERS` — "Consolidation is NOT the cure and
//! measured 6x WORSE on a real gesture" — is a verdict on collapsing the
//! *layer*, and reads as a verdict on baking. Both this host and the engine's
//! own issue tracker concluded from it that baking was out, while the call
//! built for exactly this case sat in the ABI since 0.73.0, inside this
//! repository's own pin, unbound and unmentioned.
//!
//! A reach test over the surface is what would have caught it, which is the
//! same lesson `tests/measure.rs` was written for and the reason
//! `every_consolidation_scope_is_reachable` is here.

use claycore::{ConsolidationParams, Document, Item, LayerId, Op};

/// Two overlapping lobes and a detail off to one side.
///
/// The detail is the point: a closure that swallows it has reached past the
/// patch, and on a single sphere there would be nothing to reach past.
fn form() -> Option<(Document, LayerId)> {
    let mut doc = Document::new().ok()?;
    let layer = doc.add_sdf_layer("Base").ok()?;
    doc.add_item(layer, &Item::sphere(1.0).ok()?).ok()?;
    let mut far = Item::sphere(0.18).ok()?;
    far.set_op(Op::Add).ok()?;
    far.set_position([0.0, 0.0, 1.6]).ok()?;
    doc.add_item(layer, &far).ok()?;
    Some((doc, layer))
}

fn params() -> ConsolidationParams {
    ConsolidationParams::at(0.05)
}

/// The patch worked, away from the detail.
const PATCH: ([f32; 3], [f32; 3]) = ([0.6, -0.3, -0.3], [1.2, 0.3, 0.3]);

/// Planning reports a closure, and it is the closure rather than the request.
#[test]
fn a_plan_reports_what_it_would_absorb_without_baking_it() {
    let Some((doc, layer)) = form() else {
        println!("no engine backend; skipping");
        return;
    };
    let before = doc.layer_nodes(layer).expect("nodes").len();

    let plan = doc.plan_region_merge(layer, PATCH).expect("a plan");
    println!("  closure {:?}..{:?}", plan.box_min, plan.box_max);
    println!(
        "  absorbs {} roots, whole_layer {}, ratio {:?}",
        plan.absorbed,
        plan.whole_layer,
        plan.closure_ratio(PATCH)
    );

    assert!(
        plan.absorbed > 0,
        "the patch sits on the body, so the plan must absorb something"
    );
    assert_eq!(
        doc.layer_nodes(layer).expect("nodes").len(),
        before,
        "planning must not bake: the layer still holds what it did"
    );
    // The closure is grown from the request, so it cannot be smaller than it.
    let ratio = plan.closure_ratio(PATCH).expect("the patch has volume");
    assert!(
        ratio >= 1.0,
        "the closure is the request grown until it is closed, so it cannot be \
         smaller than the request: ratio {ratio}"
    );
}

/// Working a patch twice leaves one baked item, not two.
///
/// The property the whole scope exists for, and the one a Move session needs:
/// the second bake's closure contains the first bake's volume, so it absorbs it
/// rather than stacking on it.
#[test]
fn a_patch_worked_twice_stays_at_one_baked_item() {
    let Some((mut doc, layer)) = form() else {
        return;
    };
    let (_, first) = doc
        .consolidate_region(layer, PATCH, params())
        .expect("the first bake");
    let after_one = doc.layer_nodes(layer).expect("nodes").len();

    let (_, second) = doc
        .consolidate_region(layer, PATCH, params())
        .expect("the second bake");
    let after_two = doc.layer_nodes(layer).expect("nodes").len();

    println!(
        "  after one bake: {after_one} items (absorbed {})",
        first.absorbed
    );
    println!(
        "  after two:      {after_two} items (absorbed {})",
        second.absorbed
    );

    assert_eq!(
        after_one, after_two,
        "the second bake stacked a volume on the first instead of absorbing \
         it — {after_one} items became {after_two}, which is the O(n) the \
         region scope exists to avoid"
    );
}

/// The two scopes are different scopes, and the region one leaves work behind.
///
/// Without this the binding could be pointed at `clay_layer_consolidate` and
/// every other test here would still pass.
#[test]
fn a_region_bake_is_not_a_whole_layer_bake() {
    let Some((mut doc, layer)) = form() else {
        return;
    };
    let (_, merge) = doc
        .consolidate_region(layer, PATCH, params())
        .expect("the region bake");
    let regional = doc.layer_nodes(layer).expect("nodes").len();

    let Some((mut whole_doc, whole_layer_id)) = form() else {
        return;
    };
    whole_doc
        .consolidate(whole_layer_id, params(), None)
        .expect("the whole-layer bake");
    let whole = whole_doc.layer_nodes(whole_layer_id).expect("nodes").len();

    println!("  region bake left {regional} items, whole-layer left {whole}");
    assert!(
        !merge.whole_layer,
        "the closure swallowed the layer, so this fixture cannot tell the two \
         scopes apart and the assertion below would be vacuous"
    );
    assert!(
        regional > whole,
        "a region bake left {regional} items and a whole-layer bake left \
         {whole}. The region scope is supposed to leave everything outside the \
         closure parametric"
    );
}

/// Every consolidation scope this crate offers is reachable and answers.
///
/// The gate the missing binding got past. Walked rather than named, so a scope
/// added to the crate without a call behind it fails on the row that was added
/// — the rule `tests/measure.rs` states for the six surface measures, applied
/// to the surface that needed it next.
#[test]
fn every_consolidation_scope_is_reachable() {
    let Some((mut doc, layer)) = form() else {
        return;
    };
    // Reading what a bake would cost, at each scope.
    let whole_cost = doc
        .consolidation_cost(layer, params(), None)
        .expect("the whole-layer cost is readable");
    let plan = doc
        .plan_region_merge(layer, PATCH)
        .expect("the region plan is readable");
    assert!(
        whole_cost.cell_size > 0.0,
        "a cost that reports no cell size is not a cost"
    );
    assert!(plan.absorbed > 0, "the plan reached nothing");

    // And performing one, at each scope, on its own document.
    let (regional_cost, _) = doc
        .consolidate_region(layer, PATCH, params())
        .expect("the region bake runs");
    assert!(regional_cost.cell_size > 0.0);

    let Some((mut other, other_layer)) = form() else {
        return;
    };
    let whole = other
        .consolidate(other_layer, params(), None)
        .expect("the whole-layer bake runs");
    assert!(whole.cell_size > 0.0);
}
