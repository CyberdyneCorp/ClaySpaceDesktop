# Tasks

## 1. Find the cause by measurement rather than by reading

- [x] 1.1 Build the frame guard first — every tool the field shelf offers,
      the reported gesture, roughness against an untouched sphere in the same
      run — so there is one number every later claim can be argued in
- [x] 1.2 Run the control the two withdrawal attempts did not: bake the region
      and place it **without calling the verb**. 1.02x, so the round trip is
      innocent
- [x] 1.3 Run the control that decides it: call the verb with the displacement
      set to **zero**, where `field::move_topological` returns
      `FieldVolume::sample(...)` before `solve()` and no geodesic exists. 1.88x
      against a bar of 1.5 — so the defect is in the placement, not the drag
- [x] 1.4 Sweep the feather in the deciding metric rather than in a raycast
      probe: 1.77x at one cell, 2.07x at three, 3.17x at six, 5.27x at twelve.
      It was never inert; the earlier sweep was reading the wrong instrument

## 2. Name the mechanism in the engine's own source

- [x] 2.1 `clay_item_volume_move_topological` swaps in a volume built by
      `FieldVolume::sample`, which has no feather argument, so the feather
      `bake_volume` asked for is discarded and `Op::Replace` lands hard
- [x] 2.2 Confirm it against the engine rather than asserting it: apply
      `out.set_feather(v.feather())` to a local build of the pinned ClayCore,
      with no caller change, and measure. 1.38x and a +0.2997 tip — the same
      numbers this repair reaches. Revert the engine; it stays pinned

## 3. Repair it from the caller's side

- [x] 3.1 Bake with a band that covers the drag. Without it a feathered
      placement expresses the move only up to the band: +0.0601 against +0.2997
- [x] 3.2 Place the moved volume, bake the same region back out through
      `volume_from_region`, and remove the hard placement
- [x] 3.3 Bracket the three edits into one history entry. Ungrouped the stroke
      spent four where a baked verb spends two, and the second undo put the hard
      replace back
- [x] 3.4 Move the weight's falloff from linear to smootherstep: with the
      feather live the kink reaches the shading. 1.45x → 1.38x, tip +0.2933 →
      +0.2997

## 4. Guard it

- [x] 4.1 Keep the frame guard over the whole field shelf. Reverted underneath,
      it fails by name: `Mover Topológico at 2.07x`
- [x] 4.2 Make a tool whose stroke refuses a failure rather than an early
      return, which had been disarming the guard for every tool at once
- [x] 4.3 Add `one_undo_takes_the_topological_drag_back_whole`. It guards the
      repair's own grouping and **not** the reported defect: `main` makes one
      `add_item` and passes it unchanged. Deleting the `begin_undo_group` /
      `end_undo_group` pair fails it at four entries against two
- [x] 4.4 Keep the horseshoe fixture, which is the only evidence the reach is
      genuinely geodesic
- [x] 4.5 Tighten the horseshoe's near-tip assertion from `> 0.05` to two
      thirds of the gesture, which is the scenario the delta spec already
      wrote and no test implemented. `> 0.05` passed at the band-clamped
      +0.0601, so the band half of the repair shipped **unguarded** — and the
      frame guard actively preferred the broken variant, scoring it 1.14x
      against the repair's 1.38x because a drag that moves nothing is smooth.
      Reverted underneath, it now fails by name at +0.0601
- [x] 4.6 Check the two guards are complementary rather than redundant, by
      reverting each half alone: against `main` the horseshoe passes (+0.2933,
      a hard replace has no clamp) and the frame guard fails (2.07x); with only
      the band reverted the frame guard passes (1.14x) and the horseshoe fails
      (+0.0601). Each half has exactly one guard and neither stands in for the
      other

## 5. Price it

- [x] 5.1 Measure what the band costs, which the first round of this repair
      never did: peak RSS 190 → 503 MB and 51 → 111 ms for the issue's own
      0.4 pull, 405 → 2225 MB and 461 → 799 ms for a two-unit one
- [x] 5.2 Attribute it. Holding the gesture fixed so the sampled box does not
      move and sweeping only the band gives 128 / 388 / 505 / 788 MB at bands
      of 0.06 / 0.26 / 0.46 / 1.06, and it **saturates** — 2.06 reads 683 MB,
      no worse than 1.06, because past the box's own size every brick already
      stores samples
- [x] 5.3 Refute `padding` as the cause, which a review proposed and asked to
      have pinned. Pinning it is a measured no-op (564 / 2333 MB); widening it
      alone with the band left at three cells is also a no-op (127 / 347 MB);
      and `clay_c.cpp:10471` applies `padding` only on the branch that was
      passed no region, which is not this call
- [x] 5.4 Record the price where a reader meets the code, on
      `topological_move_stroke` and beside the band, and say that the engine
      one-liner removes the second bake and the wide band together
