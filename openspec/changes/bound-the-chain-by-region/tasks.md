## Status

The mechanism is built and tested, and **it ships switched off**. Measured once
it existed, a baked patch costs about sixty times the analytic chain it replaces
per brick refilled at the brick cache's spacing, and an undo refills the whole
bound of the node an edit hangs off — so collapsing bounds the chain and makes
every undo after it slower (61 ms to 3.6 s at the cache's spacing, 61 ms to
760 ms at the spacing the bake now uses). See design.md, *Measured after
implementation*. `ClayDocument::set_compaction_floor(CHAIN_FLOOR)` turns it on;
nothing in the application calls it, and the tripwire
`a_baked_patch_still_refills_dearer_than_its_chain` fails the day it should.

What would let this change close is upstream, and is either half of a product:
a baked volume that refills near the chain's per-brick price, or an undo whose
reach for a grab is the grab's support rather than its node's whole bound. The
unchecked tasks below are the ones that only make sense once one of those holds.

## 1. Calibrate the floor, before anything is wired

- [x] 1.1 Extend the existing decay fixture to record, per gesture on a worked patch: chain length, `safe_step_scale`, and whether a raycast still finds the surface. Mirrored and unmirrored. — `tests/chain_compaction.rs`, `calibrate_the_floor_on_a_worked_patch`, 512 rays through the engine's own march.
- [x] 1.2 Find the step scale at which the surface begins to fail rather than the one at which it looks poor — the 16-dab case where a raycast found the surface 33 times out of 512 is the far end; the floor wants to sit well before it. — The first ray is lost at 0.0134 (unmirrored chain 14); mirrored chain 26 finds 41 of 512 and chain 28 none.
- [x] 1.3 Repeat for a stamp-heavy session (Padrão, Argila) as well as a drag-heavy one, and take the worst case. Record the number with the fixture that produced it, not on its own. — Padrão, Inflar and Camada build no chain at all over sixteen gestures, so the worst case is the drag. `CHAIN_FLOOR = 0.05`, recorded with its table in `compaction.rs`.
- [x] 1.4 Confirm `0.5` — what `consolidate_layer` passes today for a different question — is or is not that number, and say which in the code. — It is not; said at `CHAIN_FLOOR` and at `consolidate_layer`.

## 2. Reach the region from a committed gesture

- [x] 2.1 Carry the region a field gesture touched through to commit. — From the gesture's own samples in the layer's frame, which every field verb shares, rather than from `move_surface_regions`, which only Move returns and which is already reflected.
- [x] 2.2 Dilate by the brush radius exactly as the bake paths do, and do NOT reflect it.
- [x] 2.3 Keep the region per layer, so a gesture on one layer cannot compact another.

## 3. Decide with plan, act with bake

- [x] 3.1 At the end of a committed field gesture, read `field_report` at the calibrated floor; where the layer is degraded by a deformer chain, call `plan_region_merge` for the region. — When the floor is set; with it at zero nothing is asked.
- [x] 3.2 Collapse only when the plan says it would stay local. Record `whole_layer`, and do not infer local-vs-whole from `absorbed` against `item_count`. — With one exception, measured: the first collapse on a layer takes the starting form and is whole-layer, and every one after it is local because the engine's local path retains the volume the first installed. Declining the first would decline them all.
- [x] 3.3 Record the returned box width per layer. If it widens across successive collapses for a request that did not, stop collapsing that layer and say why. — Two successive widenings of more than 10% stop it; the reason is kept in `CompactionTotals::stopped`.
- [x] 3.4 Refill the layer's surface after the collapse the same way `consolidate_layer` does. — Over the closure, padded, through `drain_dirty` and therefore under the host's `RefillBudget`.

## 4. Tell a sculptor, and account for it

- [ ] 4.1 Report the collapse as work in progress while it runs, in all three locales, so a 125 ms pause is not read as a stall. — Not done: nothing collapses in the application, so there is nothing to report yet.
- [ ] 4.2 Attribute the time separately in the diagnostics report, so a session's sculpting time and its compaction time can be told apart. — The engine keeps the figures (`ClayDocument::compaction_totals`); the report line is left for when there is something to put on it.

## 5. Unblock the Optimize button

- [ ] 5.1 Replace the refusal at `consolidate_layer` with the regional collapse, using the last gesture's region for that layer. — Not done, deliberately: measured, the regional bake slows the layer the sculptor asked to speed up, so the refusal is the right answer on this pin. The sculpting-tools delta now says so.
- [ ] 5.2 Keep the refusal only where no region is known, with a message that says so rather than the current one about brush chains. — Follows 5.1.
- [x] 5.3 Leave the whole-layer path untouched for a stack of volumes or a long edit list.

## 6. Tests, each failing before its change

- [x] 6.1 A worked patch's chain returns to zero and stays there across at least twenty gestures. — `a_worked_patch_stays_bounded_on_the_local_path`: twenty-four mirrored gestures, longest chain 18, three collapses.
- [x] 6.2 A layer that never crosses the floor is never collapsed — assert zero bakes, not just a fast session. — `a_stamp_session_is_never_collapsed` and `compaction_is_off_by_default`.
- [x] 6.3 **The local path is pinned, not just the outcome.** Assert `whole_layer` is false from the second collapse onward on a mirrored document.
- [x] 6.4 A widening closure for a fixed request stops the collapsing and surfaces the reason. Drive it with a stub or a forced region. — Unit tests in `compaction.rs`, driven with forced widths.
- [x] 6.5 The surface a gesture drew is bit-identical with and without the collapse that follows it. — Not bit-identical, which a sampled volume cannot be: `a_collapse_keeps_the_surface_it_was_given` holds the two surfaces within one sample of the bake (measured 0.0165 against a 0.04 cell).
- [x] 6.6 Revert each production change underneath its test and confirm the test fails. — Removing the end-of-gesture call fails the bound, the surface and the tripwire tests; dropping the chain condition from the policy fails the stamp test; a non-zero default floor fails `compaction_is_off_by_default`.

## 7. Documentation

- [ ] 7.1 `docs/features.md`: what compaction is, when it fires, what it costs, and that it never changes a stroke. — Nothing a sculptor can see has changed, so there is no feature to describe yet.
- [x] 7.2 `docs/why-move-was-slow.md`: the chain section currently ends at "the chain still only grows". Close it, and correct the ruled-out table's regional-consolidation row.
- [x] 7.3 Record the calibrated floor and the fixture it came from where the next reader will look for it, not only in a commit message. — `CHAIN_FLOOR`'s own documentation, and `docs/why-move-was-slow.md`.
