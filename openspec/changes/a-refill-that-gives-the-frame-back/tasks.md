## 1. Give one drain a budget

- [x] 1.1 `RefillBudget` — the whole drain, a duration, or a number of bricks — says what one call may spend. The whole drain is the default, because a host that does not pump is a host whose surface would never catch up and a silently stale surface is worse than a slow one.
- [x] 1.2 `RefillBudget::next_batch` prices the next batch on what the batches before it in the same drain cost, with a floor: a batch pays a fixed device submission, and a budget run down to a handful of bricks would spend the frame's share on that and make no progress.
- [x] 1.3 `drain_dirty` asks for the batch size *before* taking the bricks, since a batch in hand is out of the cache and stopping there would mean keeping a second dirty set of our own.
- [x] 1.4 `refill_batch` carries the backend routing and the calibration split out of the loop, so the loop is the budget and nothing else.
- [x] 1.5 `refill_is_pending`, `pump_refill` and `settle_refill` are the continuation. Settling puts the budget back: it belongs to the host, not to whichever operation last needed an exact answer.

## 2. Pump it where the frames are

- [x] 2.1 `main` sets half a frame as the budget, where a frame starts existing and nowhere else. A benchmark, a test and the reference builder keep the whole drain.
- [x] 2.2 `App::pump_refill` spends one budget at the top of each frame, before the geometry sync reads the dirty set, and asks for the next frame while there is more — the loop waits on events, so a pump that did not ask would leave the rest until somebody moved the mouse.
- [x] 2.3 `outstanding_work` names an outstanding refill, for the same reason it names the pending re-mesh: an agent measuring the surface has to know it is still catching up.

## 3. Retire a sweep by its own bound

- [x] 3.1 `retire_curve_node` marks the node's own bound before the removal and drains after it, instead of refilling the layer whole and then the box the layer vacated.
- [x] 3.2 Say why the bound is the node's and not the control points' boxes: a drag can afford boxes that are a little short of the truth because a later segment covers the gap, and a retire cannot — measured, the points' boxes left 26 bricks holding a sweep that was gone.

## 4. Borrow a visibility pattern for free

- [x] 4.1 `with_borrowed_visibility` runs a body under a pattern it puts straight back, and nothing it writes marks the cache: the fold is the same fold on both sides.
- [x] 4.2 `with_only_visible` and the save under a solo take that bracket. Both restore before anything reads the surface, which is the whole of what makes it sound.
- [x] 4.3 A restore that did not complete is the one exit that still owes a refill, and the bracket pays it for the layers that did not come back.

## 5. Hold it

- [x] 5.1 `crates/clayspace-engine/tests/bounded_refill.rs`: `retiring_a_curve_dirties_only_its_own_region` and `a_cancelled_curve_leaves_no_tube_behind` — the second is what stops a cheaper bound that leaves the tube standing from passing the first.
- [x] 5.2 The same file: `borrowing_visibility_for_a_bake_refills_nothing` and `a_boolean_refills_only_where_its_result_stands`, the second asked of the bystanders' own bricks.
- [x] 5.3 The same file: `a_bounded_drain_stops_and_says_so`, `an_unbudgeted_drain_finishes_before_it_returns`, `pumping_a_bounded_drain_reaches_the_surface_a_whole_one_does`, `settling_finishes_what_a_budget_stopped`.
- [x] 5.4 The same file: `hiding_a_worked_subtool_is_bounded_by_the_budget`, which is the criterion the visibility work handed on — a worked field subtool's eye really does change the field, so the only way it comes under a frame is by being stopped and continued.

## 6. Say so

- [x] 6.1 `docs/features.md`: what a refill costs the frame it starts in, and that a large one finishes over the frames after it.

## 7. Left for later

- [ ] 7.1 A per-layer sampler in the engine, so a bake reads its operand instead of hiding the scene around it. What is removed here is the cost of the borrowing, not the borrowing.
- [ ] 7.2 The refill itself on the engine's worker pool. The budget brings a command under a frame; moving the work would bring the whole refill under one.
- [ ] 7.3 A figure in the interface for a refill that will take several frames. `outstanding_work` names it for an agent; a sculptor sees the surface filling in.
- [ ] 7.4 Benchmarks in `crates/clayspace-app/src/bin/bench` for a curve cancel at several radii and a boolean over two large operands, each with an interface-thread budget assertion. The regression tests hold the bound as a brick count, which is deterministic on a shared machine in a way a duration is not.
