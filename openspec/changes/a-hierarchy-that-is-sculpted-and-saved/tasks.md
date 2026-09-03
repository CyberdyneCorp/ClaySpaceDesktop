# Tasks

## 1. The two seams at the engine boundary

- [x] 1.1 `Document::multires_from_mesh_layer` — the fused borrow, because a
      layer's mesh is the document's to lend and Rust will not hold both
- [x] 1.2 `Document::replace_mesh_layer` — the only call that puts geometry
      made outside the document onto a layer without the layer changing
      identity
- [x] 1.3 Both measured against the long way round rather than assumed
      equivalent, and the replacement's stale-revision refusal exercised

## 2. Holding one

- [x] 2.1 `Layer::multires`, and the layer summary filled exactly where a
      hierarchy is held
- [x] 2.2 `crate::multires::Hierarchy` — the surface, the drawn level, the
      generation and the open gesture
- [x] 2.3 The drawn level cached against the display level and the revision,
      because copying one is 3.16 ms and a resting frame must not pay it
- [x] 2.4 A generation of this application's own, because the engine's
      restarts at one whenever a hierarchy is rebuilt from bytes

## 3. Crossing in and out

- [x] 3.1 `MeshToMultires`, refusing rather than repairing and naming the fault
- [x] 3.2 `MultiresToMesh`, baking the display level
- [x] 3.3 The row that comes out of the first carries the cage the hierarchy
      welded, taken back off it, rather than the triangles that went in

## 4. Sculpting one

- [x] 4.1 `carried_stroke` extracted, so the mesh's descriptor and the
      hierarchy's are one assembly
- [x] 4.2 `stroke_multires`, with the gesture carried into the hierarchy's own
      frame and the brush with it
- [x] 4.3 No seed, with the reason measured: the token renumbers on every bind
- [x] 4.4 A dragging verb replays from its anchor, taken back by the recorded
      bytes
- [x] 4.5 Picked against the drawn triangles rather than against the cage
- [x] 4.6 The redraw hash folds the revision and the generation, or a dab
      never reaches the screen

## 5. The side-car

- [x] 5.1 `<path>.multires`, one file, rewritten whole, priced by
      `clay_multires_preflight_encode`
- [x] 5.2 Keyed by stack position, because a `LayerKey` is minted at run time
      and a name is not unique upstream
- [x] 5.3 Written inside `save`, and a failure **fails the save**
- [x] 5.4 Read inside `from_file`, before the passes that ask a layer what it
      is
- [x] 5.5 A missing side-car opens the cage; a damaged record costs one row and
      names it
- [x] 5.6 The names reach the diagnostics report

## 6. Undo

- [x] 6.1 `GestureRecord`, so the mesh's deltas and the hierarchy's bytes order
      against the engine's history in one stack rather than two
- [x] 6.2 One blob per step, taken on the way past, so undo and redo are
      symmetric
- [x] 6.3 Bounded by bytes, oldest first

## 7. Levels

- [x] 7.1 `apply_multires_level_op` and `subdivision_cost`
- [x] 7.2 The refusal stated against the **peak**, and available before the
      button is pressed

## 8. Evidence

- [x] 8.1 The property the tier exists for: sculpt fine, move the form
      underneath, and the detail is the same height on the same vertex
      pointing somewhere else
- [x] 8.2 A save and a reopen reproduce the sculpt, both levels included
- [x] 8.3 A missing side-car, and a damaged record
- [x] 8.4 A dab lands after the caches under it were released
- [x] 8.5 One gesture is one undo, exact, and the viewport is told
- [x] 8.6 A level over budget is refused and costs nothing
- [x] 8.7 Marched output is refused as a cage, with the fault named
- [x] 8.8 `docs/features.md`
