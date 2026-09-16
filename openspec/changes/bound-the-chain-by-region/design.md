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
