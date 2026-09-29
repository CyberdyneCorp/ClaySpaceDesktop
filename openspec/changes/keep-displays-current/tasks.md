# Tasks

## 1. Counts (V8)

- [x] 1.1 `acknowledge_remesh` re-reads the counts; `refresh_stats` after the
      carried-layer buffer and a document rebuild
      (`clayspace-vm/tests/viewmodel.rs`)

## 2. Objects in a moved subtool (D13, D4)

- [x] 2.1 `Transform::place` / `unplace`, with an exact round trip
      (`clayspace-model` gizmo tests)
- [x] 2.2 `objects`, `target_transform` and `set_object_transform` in the world;
      a placement carried into the subtool (`clayspace-engine/tests/objects.rs`)

## 3. The bar at 1280 (V7)

- [x] 3.1 Fold the crossings into one button before the row would overrun
- [x] 3.2 Give the bake section's occlusion radius its own row, and fit every
      left-panel slider to its row
- [x] 3.3 Assert every crossing is reachable inside the strip, and that the
      strip starts in the same place, in three languages and five
      representations (`visual_shell.rs`)

## 4. Housekeeping

- [x] 4.1 Archive `grid-brush-radius` (V9), whose last task #288 completed

## 5. Undo against the frame it took back (I16)

- [x] 5.1 Re-measure at v0.120.1: a stroke undone through sync and settle draws
      the frame before it exactly on the starting form, and within the render
      noise floor across a smooth seam, where the residual is ClayCore #649
      (`clayspace-app/tests/visual_incremental.rs`)
