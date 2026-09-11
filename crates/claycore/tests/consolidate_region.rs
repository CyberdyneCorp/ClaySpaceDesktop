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

/// The closure is the **connected component**, and that decides what the
/// region scope can ever buy.
///
/// `plan_region_merge` grows the requested box until every item able to reach
/// inside it is wholly inside it — and reaching is transitive. An item that
/// overlaps the patch pulls in the items *it* overlaps, and so on, so the
/// closure is the whole connected run of overlapping items containing the
/// patch. Not "the items near the stroke", which is what the header warns
/// against, and not "whatever a spanning base drags in" either.
///
/// Measured here on four layers, same call, same patch:
///
/// | layer | absorbed | whole_layer | closure/requested |
/// |---|---|---|---|
/// | base sphere + 12 stamps | 13 of 13 | yes | 77.4 |
/// | chain, every neighbour touching | 13 of 13 | yes | 7.9 |
/// | eight disjoint islands | **1** of 8 | no | 1.2 |
/// | connected near group + far group | **4** of 12 | no | 2.4 |
///
/// The middle two are the pair that matter. A chain with **no spanning item at
/// all** still closes over everything, because the overlaps chain; and putting
/// one gap in that chain stops the closure dead at the gap. So a layer does
/// not need a base sphere to defeat locality — it only needs to be connected.
///
/// **Which is what a sculpted subtool is.** Stamps overlap their neighbours;
/// that is how a continuous surface is made. So on a sculpt `consolidate_region`
/// is `consolidate`, and the region scope's locality is available only across
/// disconnected pieces — a floating detail, a separate eye — and not within
/// the form itself.
#[test]
fn the_closure_is_the_connected_run_of_overlapping_items() {
    // The patch sits on the +x side, inside the near group of every fixture.
    const PATCH: ([f32; 3], [f32; 3]) = ([0.6, -0.25, -0.25], [1.1, 0.25, 0.25]);

    let build = |positions: &[[f32; 3]], radius: f32, base: bool| -> Option<(Document, LayerId)> {
        let mut doc = Document::new().ok()?;
        let layer = doc.add_sdf_layer("L").ok()?;
        if base {
            doc.add_item(layer, &Item::sphere(1.0).ok()?).ok()?;
        }
        for at in positions {
            let mut s = Item::sphere(radius).ok()?;
            s.set_op(Op::Add).ok()?;
            s.set_position(*at).ok()?;
            doc.add_item(layer, &s).ok()?;
        }
        Some((doc, layer))
    };

    // Touching: spaced under a diameter apart. Disjoint: well over.
    let touching: Vec<[f32; 3]> = (0..13)
        .map(|i| [-0.9 + i as f32 * 0.30, 0.0, 0.0])
        .collect();
    let islands: Vec<[f32; 3]> = (0..8).map(|i| [-2.0 + i as f32 * 0.9, 0.0, 0.0]).collect();
    let mut gapped: Vec<[f32; 3]> = (0..4).map(|i| [0.6 + i as f32 * 0.28, 0.0, 0.0]).collect();
    gapped.extend((0..8).map(|i| [-3.0 + i as f32 * 0.28, 0.0, 0.0]));

    let Some((connected, l1)) = build(&touching, 0.18, false) else {
        println!("no engine backend; skipping");
        return;
    };
    let plan = connected.plan_region_merge(l1, PATCH).expect("a plan");
    assert!(
        plan.whole_layer,
        "a layer whose items all touch has one connected run, so the closure          must take all of it even with no spanning item — absorbed {} of 13",
        plan.absorbed
    );

    let Some((separate, l2)) = build(&islands, 0.18, false) else {
        return;
    };
    let plan = separate.plan_region_merge(l2, PATCH).expect("a plan");
    assert!(
        !plan.whole_layer,
        "eight islands that touch nothing should not close over each other"
    );
    assert_eq!(
        plan.absorbed, 1,
        "the closure took {} islands where only the patch's own overlaps it",
        plan.absorbed
    );

    let Some((gapped_doc, l3)) = build(&gapped, 0.18, false) else {
        return;
    };
    let plan = gapped_doc.plan_region_merge(l3, PATCH).expect("a plan");
    assert_eq!(
        plan.absorbed, 4,
        "the closure took {} items where the patch's connected group is four          and a gap separates the other eight. If this ever takes all twelve,          reaching has stopped being transitive-but-bounded and the locality          this scope offers is gone",
        plan.absorbed
    );
    assert!(!plan.whole_layer);
}
