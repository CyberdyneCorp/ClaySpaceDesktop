## Context

See proposal.md — Why. Three constraints shape the approach.

**The region has to come from a gesture.** `document.rs`'s existing refusal says so: the optimise action is *"a layer-level action with no gesture behind it, and the closure of the wrong box is either the whole layer again or a patch nobody worked."* Nothing in the layer records where a sculptor has been working. Whatever computes the region has to run where the gesture is still in hand.

**Deciding is free; baking is not.** `plan_region_merge` is pure and measured upstream at 0.0002–0.0026 ms across layer shapes from one root to thirty-two roots with a thirty-two-deep chain. The bake it decides about is 43–49 ms unmirrored and 110–140 ms mirrored. Four orders of magnitude apart, which is what lets the policy plan constantly and bake rarely.

**The floor is the one number nobody has.** Everything else in this change is measured. `field_report(id, advise_below_step_scale)` already takes a floor and `0.5` is what `consolidate_layer` passes today, chosen for a different question — whether to *offer* a whole-layer collapse — and never calibrated for this one.

## Goals / Non-Goals

**Goals:**
- Bound the chain on a worked patch without a sculptor asking.
- Cost nothing on a session that never degrades a layer.
- Make a collapse that a sculptor waits for legible while it runs and attributable afterwards.
- Detect a ratcheting closure from the host, so a future pin cannot quietly reintroduce ClayCore #595.

**Non-Goals:**
- Changing what any stroke does. This runs after commit.
- Collapsing mesh, grid or hierarchy layers. The chain this bounds is a field's.
- Replacing whole-layer consolidation. It stays correct for a stack of volumes or a long edit list.
- Calibrating the floor in this design. That is a task with a measurement, not a decision to make here.

## Decisions

### Collapse past a floor, not on a cadence

Chosen over *every gesture* and *every N gestures*.

A cadence pays on layers that never degrade, and most sessions never degrade one. The threshold pays exactly when the thing it prevents is about to happen. It is affordable to evaluate only because planning is effectively free — on a cadence that argument would not be available, and the choice would be between a steady 125 ms tax and a chain that grows.

The cost is that the pause is unpredictable: it lands on whichever gesture crosses the line rather than on a known one. That is the trade-off accepted, and it is why the pause has to be visible (below).

*Alternative — every gesture:* predictable and never degrades, but an eighth of a second at every pointer-up on a mirrored document, paid on two-dab sessions too.
*Alternative — every N:* amortised but still pays on layers that never needed it, and N is exactly as uncalibrated as a floor.
*Alternative — offer it on the button only:* smallest change, but the chain still decays unless a sculptor knows to press it, which is the state we are in now.

### The region is the gesture's own bounds, dilated by the brush

The engine already returns the region a drag reached (`move_surface_regions`), and the stroke path already computes a bounding box for the baked verbs. The region handed to the collapse is that box dilated by the brush radius — the same dilation the bake paths already use — rather than a box invented at collapse time.

The mirror is *not* handled here. `include_local_warps` folds every removed grab's support into the patch with reflected centres included, which is why the mirrored closure measures 2.88 against 1.12 unmirrored. Reflecting the request ourselves would double-count.

### `whole_layer` is the discriminator, and the box width is the tripwire

ClayCore's first guidance was to compare `absorbed` against `item_count`; they corrected it, and the correction matters here. On a one-root layer — every document until a sculptor adds a subtool — `absorbed == item_count` holds for a local collapse and a whole-layer one alike. The flag is the only thing that separates them.

The box width is carried for a different job: it is the symptom ClayCore #595 produced, and a host that records it can fail its own test if a pin brings the ratchet back, rather than discovering it as a slow session.

### The first collapse on a fresh form is whole-layer, and that is fine

Measured: gesture 1 reports `whole_layer=1` at 302 ms because the closure reaches the starting form. Every collapse after is local. The policy does not special-case it — the floor will rarely be crossed at gesture 1 — but the spec records it so nobody reads a single whole-layer collapse as the ratchet returning.

## Risks / Trade-offs

**The floor is uncalibrated, and it decides everything.** Too high and a sculptor pays 125 ms constantly; too low and the form degrades before the collapse fires. The tasks calibrate it against the existing decay fixture rather than picking a number, and the risk is that the right floor differs between a drag-heavy and a stamp-heavy session. Mitigation: calibrate against the worst case, which is Move, and record the number with its fixture.

**An unpredictable pause is a worse feel than a predictable one**, and this design chose it deliberately. If it reads badly in the hand, the cadence alternatives are still open and the measurements for them are in the proposal — this decision is reversible without redoing the work.

**The mirrored path costs roughly 2.5x the unmirrored one** — 110–140 ms against 43–49 — and mirrored is the default. That is honest cost rather than waste, because the patch genuinely spans the plane, but it means the common case is the expensive one.

**Our local path depends on the starting form's volume not participating in the layer mirror.** ClayCore's gate refuses a node that does participate, and we measured that we are on the permissive side of it by construction. If a future change makes the base volume mirror-participating, this silently becomes whole-layer maintenance, which is measured **6x worse** for a chain. A test has to pin the local path, not just the outcome.

## Measured after implementation

Everything the design needed from the engine held, and the decision it rested on did not.

**What held.** The floor calibrated cleanly: the first of 512 marched rays is lost at a step scale of 0.0134, a mirrored gesture multiplies the step scale by 0.54, and a floor of 0.05 leaves two mirrored gestures of margin. Stamps build no chain at all, so a stamp-heavy session never reaches the floor. The first collapse on a fresh form reports `whole_layer` and every later one is local — on a mirrored document too — and the closure holds its width (3.80 once, then flat). The chain returns to zero and a worked patch stays bounded.

**One correction to the decision above.** The policy *does* special-case the first collapse: "collapse only when the plan stays local" read literally declines it, and then no later collapse is ever local either, because the engine's local path needs the volume the first one installs. So a whole-layer plan is accepted on a layer's first collapse and declined on every one after.

**What did not hold is the premise that a bounded chain is a cheaper layer.** The proposal measured the bake — flat, 110–140 ms — and not what the layer costs afterwards. Measured here on the starting sphere, mirrored, one patch worked forty times (`crates/clayspace-engine/tests/chain_compaction.rs` and the series recorded in `compaction.rs`):

| | no collapse | bake at the cache's 0.02 | bake at 0.04 |
|---|---:|---:|---:|
| refill, per brick | ~7 µs | ~430 µs | — |
| undo, gesture 11 | 61 ms | 3,621 ms | 760 ms |
| undo, gesture 40 | 362 ms | 5,418 ms | 858 ms |
| surface bricks after the first bake | 1,045 | 3,432 | 1,081 |

A baked patch is a sampled volume, and a refill over one costs about sixty times what it costs over the analytic chain. An undo refills what the engine reports it reached, which for a grab hung off the starting form is the form's whole bound — thousands of bricks with or without the collapse — so the per-brick price is the whole of the difference, and the collapse raises it. At the cache's own spacing the baked band also equals the cache's band exactly, and a collapsed sphere tripled its surface bricks; baking at twice the spacing fixes that and costs a tenth as much afterwards, and it is still a loss against the chain over any session length measured. The sixty is a debug-host figure: Linux CI in a release build measured the same undo at 1.9x the chain, so the tripwire asserts the direction (above 1.25x) rather than the debug magnitude.

**So the mechanism ships with its floor at zero**, the Optimize refusal stays, and a tripwire test fails when an undo over a baked patch comes within 2x of one over the chain. The two things that would change the verdict are both upstream: a sampled volume that refills near an analytic item's per-brick cost, or an undo bound for a deformer append that is the deformer's support rather than its node's whole bound. The second would help the uncollapsed chain as much as the collapsed one, and is the more direct cure for the undo cost the audit measured.

## Re-measured on v0.120.1

The engine delivered the second of the two conditions above. `clay_document_undo_bound` now reports a Move grab as the head links' balls, clamped into the node bound, instead of the whole node dilated by every earlier pull. `ClayDocument::undo` already refilled from that bound, so the application picked it up with no code change.

The twenty-edit series is now a regression test (`crates/clayspace-engine/tests/undo_series.rs`, change `measure-the-undo-series-by-factor`), and it records the two factors apart. Starting sphere, one patch, mirrored Move at brush size 0.12, Mac release, four runs:

| | gesture 1 | gesture 20 | last three against first three |
|---|---:|---:|---:|
| undo | 0.30–0.32 ms | 0.81–0.94 ms | 2.00–2.31x |
| bricks re-meshed | 126 | 144 | 1.04x |
| price of a brick | 2.4–2.5 µs | 5.6–6.5 µs | 1.93–2.24x |

The region no longer grows. What is left is the price of one brick over a longer chain, and a collapse could be credited only with that part. It cannot claim it on this pin: `a_baked_patch_has_no_decisive_undo_win` measures an undo over a baked patch at 4.4x one over the chain. So the floor stays at zero. The first condition, a sampled volume that refills near an analytic item's price, is still the one that would change the verdict.

## Re-measured on v0.126.0

The pin moved from v0.120.1 to v0.126.0 (76 engine commits; none of them names the cost of consolidation, the baked volume or the chain's evaluation), so `measure_the_collapse_against_the_chain` was run again. Mac, release, on a host sharing its cores with another build:

| brush | gesture | undo, chain | undo, collapsing | a brick, chain | a brick, collapsing |
|---|---:|---:|---:|---:|---:|
| 0.45 | 1 | 1.7 ms | 1.7 ms | 1.9 µs | 1.8 µs |
| 0.45 | 10 | 8.2 ms | 35 ms (a collapse, 9,152 bricks) | 8.4 µs | 3.8 µs |
| 0.45 | 20 | 16.3 ms | 24 ms | 16.7 µs | 24.3 µs |
| 0.45 | 40 | 13.4 ms | 185 ms (a collapse, 10,752 bricks) | 13.7 µs | 17.2 µs |
| 0.12 | 1 | 0.36 ms | 0.30 ms | 2.5 µs | 2.1 µs |
| 0.12 | 10 | 0.89 ms | 76 ms (a collapse, 6,384 bricks) | 6.2 µs | 11.9 µs |
| 0.12 | 20 | 0.70 ms | 76 ms (a collapse) | 4.8 µs | 11.9 µs |
| 0.12 | 40 | 1.23 ms | 257 ms (a collapse) | 8.5 µs | 40.3 µs |

The verdict is the v0.120.1 one. The chain's undo stays in the millisecond class over forty gestures on a region that does not grow, and the collapse costs more at every checkpoint past the first gesture: 1.5x between collapses, 4x to 210x on a gesture that collapsed. The thirteen pinned tests in `tests/undo_series.rs` and `tests/chain_compaction.rs` pass on this pin. The floor stays at zero, and the change is archived with the mechanism built, tested and off, and with the five tasks that only make sense once a baked brick is cheap left unchecked below their reason.
