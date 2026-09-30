## Why

The rest of the #196 audit that is interface-layer and reproducible: displays
that showed what used to be true.

- **V8.** The geometry panel was one update behind. The ViewModel read the
  counts when an edit landed, and the re-mesh the edit asked for recorded new
  ones afterwards; a remesh or a display change of a carried layer moves no
  brick at all, so nothing re-read them until the next edit.
- **D13, and D4 with it.** An object's outline, its manipulator and its
  position readout were drawn from the node's own values, which are in its
  subtool's frame. Once the subtool moved or stretched, all three stayed where
  the object used to stand — the stale box of D13 — and a placement aimed at
  the world landed as far from the aim as the subtool had moved.
- **V7.** At 1280 the representation bar's crossings ran past the visible strip
  in every language, by up to 470 pixels from a mesh layer. Two causes: five
  cards and the crossing row do not fit that width, and on a mesh layer the
  bake section put two sliders on one row, which overran the left panel and
  took about 170 pixels from the central region. Spanish and Portuguese labels
  overran it by another 12 to 22 on the UV and retopology rows.
- **V9.** `grid-brush-radius` was complete but for re-recording the benchmark
  baseline, which #288 did on the runners (`voxel-reference` r3 on both
  platforms). It is archived alongside this change.
- **I16.** Re-measured at ClayCore v0.120.1 rather than fixed in the
  application. A stroke undone through the application's own sync-and-settle
  path draws the frame from before it, exactly, on the starting form. Across a
  smooth seam with a sibling placed after the form (the bound ClayCore #650
  fixed) the undo is exact on a workstation (2 pixels), but on the `macos-14`
  runners the frame *before* the edit is 334 pixels at up to 16 levels off, in
  a ring on the sibling's silhouette, while the undone frame is right: a brick
  build disagreeing with a later refill of the same field inside the blend
  band, the class ClayCore #649 leaves open. That case is bounded at the
  render noise floor rather than held exact, and nothing here pads the
  engine's bounds.
- **I14.** Not reproducible on current builds, and explained. The audit
  measured `set_grid_display` boxes to smooth at 169–349 ms "with no visible
  grid layer" on `eeb158a`, which smoothed every grid on a display change,
  hidden ones included (V5). #239 stopped that. Through the real application
  over the door, with a window and a Metal renderer, a display change costs
  0.003–0.005 ms and uploads nothing with no grid at all, and 0.002–0.028 ms
  with six sculpted grids hidden. Shown, the same six cost 51–52 ms and
  1.5 MB, which is the audit's order of magnitude for a handful of grids. The
  change measured over a plain call mid-stroke, and with thirteen field
  layers and a grid added and removed, stayed at 0.7–22 ms round trip with
  nothing uploaded by the display change itself. A tripwire test holds the
  hidden case.

## What Changes

- `SculptViewModel::acknowledge_remesh` re-reads the counts, and a new
  `refresh_stats` is called after the carried-layer buffer and after a document
  rebuild.
- The engine's object boundary speaks world coordinates: `objects()` and the
  manipulator's `target_transform` compose the node with its subtool's
  placement, `set_object_transform` and the manipulator write through its
  inverse, and a placement point is carried into the subtool. The object table
  and the document keep the node's own values. `Transform::place` / `unplace`
  in the model do the composition: position exact, rotation composed, scale per
  axis — exact unless a turned object sits in a non-uniformly stretched
  subtool, which no position, rotation and three factors can state — and an
  exact round trip either way. A subtool at its default placement is untouched.
- The representation bar gains a fourth rung: when the crossing row does not
  fit beside the cards, it folds into one "Convert…" button that opens the
  conversion panel, which offers the same crossings by name. Cards still keep
  icon and name; past the fold the bar scrolls, as before.
- Left-panel slider rows shorten their track to what is left of the row, and
  the bake section's occlusion radius has a row of its own, so no row widens
  the panel.

## Capabilities

### Modified Capabilities
- `scene-and-layers`: counts follow a rebuild that moves no brick.
- `object-transform`: an object is shown, read and manipulated where it stands.
- `representation-modes`: the bar's ladder folds the crossings before scrolling.
- `app-shell`: a panel's rows fit its width.

## Impact

- `crates/clayspace-vm/src/sculpt_vm.rs`, `crates/clayspace-app/src/main.rs`
- `crates/clayspace-model/src/gizmo.rs`, `crates/clayspace-engine/src/document.rs`
- `crates/clayspace-view/src/shell/workspace.rs`, `crates/clayspace-view/src/shell/left.rs`
- Tests: `clayspace-vm/tests/viewmodel.rs`, `clayspace-engine/tests/objects.rs`,
  the `clayspace-model` gizmo unit tests, `clayspace-app/tests/visual_shell.rs`,
  `clayspace-app/tests/visual_incremental.rs` (I16).

Not in this change, and still open on #196: D14 (first gizmo drag after
`set_mode`), I14 (`set_grid_display` with no grid layer) and D9 (a curve on a
stretched subtool). Each needs a profile or a reproduction in the running
application.
