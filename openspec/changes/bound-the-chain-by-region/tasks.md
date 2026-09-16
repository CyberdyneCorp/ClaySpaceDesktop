## 1. Calibrate the floor, before anything is wired

- [ ] 1.1 Extend the existing decay fixture to record, per gesture on a worked patch: chain length, `safe_step_scale`, and whether a raycast still finds the surface. Mirrored and unmirrored.
- [ ] 1.2 Find the step scale at which the surface begins to fail rather than the one at which it looks poor — the 16-dab case where a raycast found the surface 33 times out of 512 is the far end; the floor wants to sit well before it.
- [ ] 1.3 Repeat for a stamp-heavy session (Padrão, Argila) as well as a drag-heavy one, and take the worst case. Record the number with the fixture that produced it, not on its own.
- [ ] 1.4 Confirm `0.5` — what `consolidate_layer` passes today for a different question — is or is not that number, and say which in the code.

## 2. Reach the region from a committed gesture

- [ ] 2.1 Carry the region a field gesture touched through to commit. The baked verbs already compute a bounding box and `move_surface_regions` already returns what a drag reached; use those rather than a new box.
- [ ] 2.2 Dilate by the brush radius exactly as the bake paths do, and do NOT reflect it — `include_local_warps` folds reflected supports in, and reflecting here would double-count.
- [ ] 2.3 Keep the region per layer, so a gesture on one layer cannot compact another.

## 3. Decide with plan, act with bake

- [ ] 3.1 At the end of a committed field gesture, read `field_report` at the calibrated floor; where the layer is degraded by a deformer chain, call `plan_region_merge` for the region.
- [ ] 3.2 Collapse only when the plan says it would stay local. Record `whole_layer`, and do not infer local-vs-whole from `absorbed` against `item_count` — it is a tautology on a one-root layer.
- [ ] 3.3 Record the returned box width per layer. If it widens across successive collapses for a request that did not, stop collapsing that layer and say why.
- [ ] 3.4 Refill the layer's surface after the collapse the same way `consolidate_layer` does.

## 4. Tell a sculptor, and account for it

- [ ] 4.1 Report the collapse as work in progress while it runs, in all three locales, so a 125 ms pause is not read as a stall.
- [ ] 4.2 Attribute the time separately in the diagnostics report, so a session's sculpting time and its compaction time can be told apart.

## 5. Unblock the Optimize button

- [ ] 5.1 Replace the refusal at `consolidate_layer` with the regional collapse, using the last gesture's region for that layer.
- [ ] 5.2 Keep the refusal only where no region is known, with a message that says so rather than the current one about brush chains.
- [ ] 5.3 Leave the whole-layer path untouched for a stack of volumes or a long edit list.

## 6. Tests, each failing before its change

- [ ] 6.1 A worked patch's chain returns to zero and stays there across at least twenty gestures. Fails today: the chain grows monotonically.
- [ ] 6.2 A layer that never crosses the floor is never collapsed — assert zero bakes, not just a fast session.
- [ ] 6.3 **The local path is pinned, not just the outcome.** Assert `whole_layer` is false from the second collapse onward on a mirrored document. This is the test that catches the base volume becoming mirror-participating, which would silently turn this into whole-layer maintenance — measured 6x worse.
- [ ] 6.4 A widening closure for a fixed request stops the collapsing and surfaces the reason. Drive it with a stub or a forced region rather than waiting for a bad pin.
- [ ] 6.5 The surface a gesture drew is bit-identical with and without the collapse that follows it.
- [ ] 6.6 Revert each production change underneath its test and confirm the test fails. Record what was seen; four tests in this repository have been found asserting the defect they were written to catch.

## 7. Documentation

- [ ] 7.1 `docs/features.md`: what compaction is, when it fires, what it costs, and that it never changes a stroke.
- [ ] 7.2 `docs/why-move-was-slow.md`: the chain section currently ends at "the chain still only grows". Close it, and correct the ruled-out table's regional-consolidation row, which already carries one correction and now has an outcome.
- [ ] 7.3 Record the calibrated floor and the fixture it came from where the next reader will look for it, not only in a commit message.
