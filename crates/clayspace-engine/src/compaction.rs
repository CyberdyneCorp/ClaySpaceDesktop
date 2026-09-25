//! Bounding a field layer's deformer chain by baking the patch that was worked.
//!
//! A field layer's chain only ever grows. Each Move dab appends one grab per
//! mirror image, `safe_step_scale` is a product over the chain, and so a worked
//! patch decays geometrically: every dab, every refill and every undo on that
//! layer pays for every grab that came before it. The audit measured an undo on
//! a flat 283k-triangle layer go from 20 ms to 4.5 s over twenty edits while
//! the geometry did not grow at all.
//!
//! The cure on paper is `clay_layer_consolidate_region`, which bakes the
//! influence closure of a box into one volume and leaves the rest parametric.
//! This module is the *policy* around it and nothing else — which gestures are
//! asked about, when a bake is due, and when to stop trusting the engine's
//! closure — kept apart from `document.rs` so that it can be driven without an
//! engine. The engine calls live in [`crate::ClayDocument`], which feeds this
//! the plan and the merge and does what it says.
//!
//! **Deciding is free and baking is not.** `plan_region_merge` is pure and
//! measured upstream at thousandths of a millisecond; the bake it decides about
//! is tens to hundreds. That gap is what lets the policy ask at the end of every
//! field gesture and bake only when the layer has actually degraded.
//!
//! # Why it is off
//!
//! **The floor is zero until a host sets one, and nothing in the application
//! sets one.** Everything above works — the chain returns to zero, the first
//! collapse takes the starting form and every one after it is local, the
//! closure holds its width — but an undo over the baked patch remains dearer
//! than over the chain on v0.120.1. The historical series below was measured
//! against v0.120.0 on the starting sphere, mirrored, one patch worked forty
//! times (`tests/chain_compaction.rs`, Mac, debug host over a Release engine):
//!
//! | | no collapse | collapse at the cache's 0.02 | collapse at 0.04 |
//! |---|---:|---:|---:|
//! | refill, per brick | ~7 µs | ~430 µs | — |
//! | undo, gesture 9 | 50 ms | 50 ms | 145 ms |
//! | undo, gesture 11 | 61 ms | **3,621 ms** | 760 ms |
//! | undo, gesture 40 | 362 ms | 5,418 ms | 858 ms |
//! | gesture, steady | 20–54 ms, rising | 80–90 ms | 45 ms |
//!
//! A baked patch is a sampled volume, and a refill on v0.120.0 cost about
//! sixty times as much per brick over one as over the analytic chain it
//! replaced. An undo then refilled the starting form's whole bound. The
//! v0.120.1 bound is narrower, but its measured undo still favors the chain. The
//! chain *is* bounded; the cost the chain stood for is not, over the range a
//! session reaches. Those are debug-host figures: Linux CI in release measured
//! the gesture-11 undo at 1.9x the chain rather than sixty, so the size of the
//! loss depends on the build and its direction does not. The engine measured
//! whole-layer consolidation 6x worse for a chain on the same grounds, and the
//! regional scope inherits it because what it installs is the same kind of
//! item.
//!
//! v0.120.1 narrows an undo for a grab to the grab's support, but the tripwire
//! `a_baked_patch_has_no_decisive_undo_win` measures the baked patch against
//! the chain: local macOS debug and release found about 4.9x, while loaded CI
//! runners have varied through near parity. Only a large win triggers a new
//! policy review; the floor stays off by default until repeatable measurements
//! justify enabling it.

use std::collections::HashMap;
use std::time::Duration;

use clayspace_model::LayerKey;

/// A world box as the engine takes one: minimum corner, maximum corner.
pub type Bounds = ([f32; 3], [f32; 3]);

/// The step scale under which a chain-degraded layer is collapsed, once
/// collapsing is turned on.
///
/// Calibrated rather than chosen, and not `0.5` — the number
/// `consolidate_layer` passes to `field_report` to decide whether to *offer* a
/// whole-layer bake, which is a different question. Measured on the starting
/// sphere (`tests/chain_compaction.rs`, `calibrate_the_floor_on_a_worked_patch`)
/// with one patch worked by repeated Move gestures and the front-only gate on,
/// which is the setting that degrades a layer fastest, counting how many of 512
/// rays marched at the patch still arrive:
///
/// | mirrored chain | unmirrored chain | safe step scale | rays arriving |
/// |---:|---:|---:|---:|
/// | 2 | 1 | 0.735 | 512 |
/// | 8 | 4 | 0.292 | 512 |
/// | 16 | 8 | 0.085 | 512 |
/// | 20 | 10 | 0.034–0.046 | 512 |
/// | — | 14 | 0.0134 | **510** |
/// | 24 | 15 | 0.0099 | 274 / 438 |
/// | 26 | 16 | 0.0053 / 0.0073 | 41 / 260 |
/// | 28 | — | 0.0029 | **0** |
///
/// The first ray is lost at 0.0134, and the step scale falls by 0.735 per link
/// — 0.54 per mirrored gesture. The floor sits at 0.05, so a layer found just
/// above it survives two more mirrored gestures, 0.05 to 0.0146, before it
/// could lose a ray: one collapse can be declined or deferred and the surface
/// still holds. Stamps (Padrão, Inflar, Camada) built no chain at all over
/// sixteen gestures — a stamp is an item, not a link — so a stamp-heavy session
/// never reaches this floor however far its step scale falls; that is the
/// whole-layer bake's case, and it keeps it.
pub const CHAIN_FLOOR: f32 = 0.05;

/// How many successive widenings of the closure, for a request that did not
/// widen, stop the collapsing on a layer.
///
/// Two rather than one because the first collapse on a fresh form is allowed
/// to be the widest: it absorbs the starting form, and the collapses after it
/// are local. One widening is that; two in a row is the ratchet ClayCore #595
/// was, and a policy that kept paying for it would compound.
pub const RATCHET_STRIKES: u8 = 2;

/// How much a closure may grow between two collapses before it counts as
/// widening, as a fraction of its previous width.
///
/// Wide enough to absorb what one patch worked repeatedly measures here —
/// 3.80 for the first collapse, then 3.20 or 3.84 at every one after, flat —
/// and narrow enough that the #595 ratchet, about 1.76x a bake, cannot hide
/// under it.
const WIDENING: f32 = 0.10;

/// Why a gesture's region was not collapsed.
#[derive(Debug, Clone, PartialEq)]
pub enum Declined {
    /// The layer is above the floor, or what is costing it is not a chain.
    /// The overwhelmingly common answer, and the one that costs nothing.
    Healthy,
    /// The plan says the closure would take the whole layer, so the bake would
    /// be the whole-layer consolidation under another name — measured 6x worse
    /// for a deformer chain.
    WouldNotStayLocal { absorbed: u64 },
    /// Collapsing on this layer was stopped because its closure kept widening
    /// for a request that did not.
    Stopped(Ratchet),
}

/// A closure seen to widen, with the widths that showed it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ratchet {
    pub from: f32,
    pub to: f32,
}

/// What one collapse did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Collapse {
    pub layer: LayerKey,
    /// The merge's own local-vs-whole answer. Not inferred from `absorbed`
    /// against the layer's root count: on a one-root layer — every document
    /// until a sculptor adds a subtool — those are equal for a local collapse
    /// and a whole-layer one alike.
    pub whole_layer: bool,
    pub absorbed: u64,
    /// The largest extent of the box that was asked for.
    pub request_width: f32,
    /// The largest extent of the closure the engine baked.
    pub closure_width: f32,
    /// The bake alone. The refill after it goes through the drain like any
    /// other and is counted there.
    pub took: Duration,
}

/// What the session has spent on collapsing, apart from what it spent
/// sculpting.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompactionTotals {
    pub collapses: u32,
    pub whole_layer: u32,
    pub declined_non_local: u32,
    pub time: Duration,
    pub longest: Duration,
    /// Layers collapsing was stopped on, with why.
    pub stopped: Vec<(LayerKey, Ratchet)>,
}

/// One layer's side of the policy.
#[derive(Debug, Default)]
struct LayerState {
    /// The region the most recent gesture on this layer touched, in the
    /// layer's own frame, dilated by the brush.
    worked: Option<Bounds>,
    /// Whether the gesture that set `worked` is still open, so a further
    /// segment grows it rather than replacing it.
    open: bool,
    /// Whether a gesture has touched this layer since the last end of one.
    pending: bool,
    /// The widths the last collapse was asked for and baked.
    last: Option<(f32, f32)>,
    strikes: u8,
    collapses: u32,
    stopped: Option<Ratchet>,
}

/// The policy's state for a whole document.
#[derive(Debug, Default)]
pub struct Compaction {
    layers: HashMap<LayerKey, LayerState>,
    /// Zero by default, which is off. See the module's own account of why.
    floor: f32,
    totals: CompactionTotals,
    /// What the last decision that reached a plan came to.
    last: Option<Result<Collapse, Declined>>,
}

impl Compaction {
    /// The floor in force.
    pub fn floor(&self) -> f32 {
        self.floor
    }

    /// Whether the end of a gesture asks anything at all.
    pub fn is_enabled(&self) -> bool {
        self.floor > 0.0
    }

    /// Sets the floor. Zero turns end-of-gesture collapsing off.
    pub fn set_floor(&mut self, floor: f32) {
        self.floor = floor.max(0.0);
    }

    pub fn totals(&self) -> &CompactionTotals {
        &self.totals
    }

    /// What the last decision that reached a plan came to: the collapse, or
    /// why there was none. A layer found healthy is not recorded here — that
    /// is the answer at the end of almost every gesture, and it would bury the
    /// one worth reading.
    pub fn last(&self) -> Option<&Result<Collapse, Declined>> {
        self.last.as_ref()
    }

    /// Notes that a stroke landed on `layer` over `region`.
    ///
    /// Per layer, so a gesture on one subtool can never compact another.
    pub fn note_stroke(&mut self, layer: LayerKey, region: Bounds) {
        let state = self.layers.entry(layer).or_default();
        state.worked = Some(match (state.open, state.worked) {
            (true, Some(held)) => union(held, region),
            _ => region,
        });
        state.open = true;
        state.pending = true;
    }

    /// Ends the gesture: every region it grew is closed, and the layers it
    /// touched are handed back with those regions, for the caller to decide
    /// about.
    pub fn end_gesture(&mut self) -> Vec<(LayerKey, Bounds)> {
        let mut touched = Vec::new();
        for (key, state) in &mut self.layers {
            state.open = false;
            if std::mem::take(&mut state.pending) {
                if let Some(region) = state.worked {
                    touched.push((*key, region));
                }
            }
        }
        touched.sort_by_key(|(key, _)| key.0);
        touched
    }

    /// The last gesture's region on `layer`: what a collapse of that layer
    /// would ask about.
    pub fn worked(&self, layer: LayerKey) -> Option<Bounds> {
        self.layers.get(&layer).and_then(|state| state.worked)
    }

    /// Whether a layer at this step scale, degraded this way, is due a
    /// collapse at all. The half of the decision that needs no plan.
    pub fn is_due(&self, safe_step_scale: f32, chain_degraded: bool) -> bool {
        chain_degraded && safe_step_scale < self.floor
    }

    /// Whether the plan the engine returned for `layer` may be baked.
    ///
    /// A plan that would take the whole layer is declined, except on a layer
    /// that has never been collapsed: the first collapse on a fresh form
    /// reaches the starting form, which is one root, so it *is* the whole
    /// layer, and every collapse after it is local — measured, on the starting
    /// sphere with the mirror on, the first reports `whole_layer` and none of
    /// the eight after it does. Declining that one would decline every one
    /// after it too, since there would be no baked volume for the engine's
    /// local path to retain.
    pub fn judge(&self, layer: LayerKey, whole_layer: bool, absorbed: u64) -> Result<(), Declined> {
        let state = self.layers.get(&layer);
        if let Some(ratchet) = state.and_then(|state| state.stopped) {
            return Err(Declined::Stopped(ratchet));
        }
        let first = state.is_none_or(|state| state.collapses == 0);
        if whole_layer && !first {
            return Err(Declined::WouldNotStayLocal { absorbed });
        }
        Ok(())
    }

    /// Records a decline, so the diagnostics can count what was not done.
    pub fn declined(&mut self, reason: &Declined) {
        if matches!(reason, Declined::WouldNotStayLocal { .. }) {
            self.totals.declined_non_local += 1;
        }
        if *reason != Declined::Healthy {
            self.last = Some(Err(reason.clone()));
        }
    }

    /// Records a collapse, and stops collapsing the layer if its closure has
    /// widened too often for a request that did not.
    pub fn record(&mut self, collapse: Collapse) {
        let state = self.layers.entry(collapse.layer).or_default();
        state.collapses += 1;
        let widened = state.last.is_some_and(|(request, closure)| {
            collapse.request_width <= request * (1.0 + WIDENING)
                && collapse.closure_width > closure * (1.0 + WIDENING)
        });
        if widened {
            state.strikes += 1;
        } else {
            state.strikes = 0;
        }
        if state.strikes >= RATCHET_STRIKES {
            let from = state.last.map_or(0.0, |(_, closure)| closure);
            let ratchet = Ratchet {
                from,
                to: collapse.closure_width,
            };
            state.stopped = Some(ratchet);
            self.totals.stopped.push((collapse.layer, ratchet));
        }
        state.last = Some((collapse.request_width, collapse.closure_width));

        let totals = &mut self.totals;
        totals.collapses += 1;
        totals.whole_layer += u32::from(collapse.whole_layer);
        totals.time += collapse.took;
        totals.longest = totals.longest.max(collapse.took);
        self.last = Some(Ok(collapse));
    }

    /// Why collapsing stopped on `layer`, if it did.
    pub fn stopped(&self, layer: LayerKey) -> Option<Ratchet> {
        self.layers.get(&layer).and_then(|state| state.stopped)
    }
}

/// The largest extent of a box: the one width a ratchet shows up in.
pub fn width((min, max): Bounds) -> f32 {
    (0..3).map(|axis| max[axis] - min[axis]).fold(0.0, f32::max)
}

/// A box grown by `by` on every side.
pub fn dilated((min, max): Bounds, by: f32) -> Bounds {
    (
        std::array::from_fn(|axis| min[axis] - by),
        std::array::from_fn(|axis| max[axis] + by),
    )
}

/// The box around a set of points.
pub fn around(points: impl IntoIterator<Item = [f32; 3]>) -> Option<Bounds> {
    points.into_iter().fold(None, |held, point| {
        Some(match held {
            Some(bounds) => union(bounds, (point, point)),
            None => (point, point),
        })
    })
}

fn union((a_min, a_max): Bounds, (b_min, b_max): Bounds) -> Bounds {
    (
        std::array::from_fn(|axis| a_min[axis].min(b_min[axis])),
        std::array::from_fn(|axis| a_max[axis].max(b_max[axis])),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYER: LayerKey = LayerKey(1);

    fn collapse(request_width: f32, closure_width: f32, whole_layer: bool) -> Collapse {
        Collapse {
            layer: LAYER,
            whole_layer,
            absorbed: 1,
            request_width,
            closure_width,
            took: Duration::from_millis(100),
        }
    }

    #[test]
    fn a_gesture_grows_its_region_and_the_next_one_replaces_it() {
        let mut policy = Compaction::default();
        policy.note_stroke(LAYER, ([0.0; 3], [1.0; 3]));
        policy.note_stroke(LAYER, ([2.0; 3], [3.0; 3]));
        assert_eq!(policy.worked(LAYER), Some(([0.0; 3], [3.0; 3])));
        assert_eq!(policy.end_gesture(), vec![(LAYER, ([0.0; 3], [3.0; 3]))]);
        // Nothing touched since: nothing to decide about.
        assert!(policy.end_gesture().is_empty());
        // Still known, for the Optimize action.
        assert_eq!(policy.worked(LAYER), Some(([0.0; 3], [3.0; 3])));

        policy.note_stroke(LAYER, ([5.0; 3], [6.0; 3]));
        assert_eq!(policy.worked(LAYER), Some(([5.0; 3], [6.0; 3])));
    }

    #[test]
    fn a_gesture_on_one_layer_hands_back_only_that_layer() {
        let mut policy = Compaction::default();
        policy.note_stroke(LayerKey(2), ([0.0; 3], [1.0; 3]));
        let touched = policy.end_gesture();
        assert_eq!(touched.len(), 1);
        assert_eq!(touched[0].0, LayerKey(2));
        assert_eq!(policy.worked(LAYER), None);
    }

    #[test]
    fn it_is_off_until_a_floor_is_set() {
        let mut policy = Compaction::default();
        assert!(!policy.is_enabled());
        assert!(
            !policy.is_due(0.0001, true),
            "a zero floor is never crossed"
        );
        policy.set_floor(CHAIN_FLOOR);
        assert!(policy.is_enabled());
    }

    #[test]
    fn only_a_chain_under_the_floor_is_due() {
        let mut policy = Compaction::default();
        policy.set_floor(CHAIN_FLOOR);
        assert!(policy.is_due(CHAIN_FLOOR * 0.5, true));
        assert!(!policy.is_due(CHAIN_FLOOR * 2.0, true));
        assert!(
            !policy.is_due(0.01, false),
            "a layer degraded by volumes is the whole-layer bake's case"
        );
    }

    #[test]
    fn the_first_collapse_may_take_the_whole_layer_and_later_ones_may_not() {
        let mut policy = Compaction::default();
        assert_eq!(policy.judge(LAYER, true, 1), Ok(()));
        policy.record(collapse(1.0, 2.45, true));
        assert_eq!(
            policy.judge(LAYER, true, 1),
            Err(Declined::WouldNotStayLocal { absorbed: 1 })
        );
        assert_eq!(policy.judge(LAYER, false, 1), Ok(()));
    }

    /// The ratchet, driven directly: the engine's closure widening at about
    /// the rate ClayCore #595 measured, for a request that stays put.
    #[test]
    fn a_widening_closure_for_a_fixed_request_stops_the_collapsing() {
        let mut policy = Compaction::default();
        policy.record(collapse(1.0, 2.0, false));
        policy.record(collapse(1.0, 3.5, false));
        assert_eq!(
            policy.judge(LAYER, false, 1),
            Ok(()),
            "one widening is allowed"
        );
        policy.record(collapse(1.0, 6.2, false));
        let stopped = policy.judge(LAYER, false, 1);
        assert_eq!(
            stopped,
            Err(Declined::Stopped(Ratchet { from: 3.5, to: 6.2 })),
            "two successive widenings are the ratchet"
        );
        assert_eq!(
            policy.totals().stopped,
            vec![(LAYER, Ratchet { from: 3.5, to: 6.2 })]
        );
    }

    #[test]
    fn a_closure_that_widens_because_the_request_did_is_not_a_ratchet() {
        let mut policy = Compaction::default();
        policy.record(collapse(1.0, 2.0, false));
        policy.record(collapse(2.0, 4.0, false));
        policy.record(collapse(4.0, 8.0, false));
        assert_eq!(policy.judge(LAYER, false, 1), Ok(()));
    }

    #[test]
    fn a_flat_closure_keeps_collapsing() {
        let mut policy = Compaction::default();
        policy.record(collapse(1.0, 2.45, true));
        for _ in 0..20 {
            policy.record(collapse(1.0, 2.88, false));
        }
        assert_eq!(policy.judge(LAYER, false, 1), Ok(()));
        assert_eq!(policy.totals().collapses, 21);
        assert_eq!(policy.totals().whole_layer, 1);
    }
}
