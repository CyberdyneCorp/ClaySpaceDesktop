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
- [x] 4.3 Add `one_undo_takes_the_topological_drag_back_whole`. Reverted
      underneath, it fails: four entries against two
- [x] 4.4 Keep the horseshoe fixture, which is the only evidence the reach is
      genuinely geodesic, and which is what caught the band
