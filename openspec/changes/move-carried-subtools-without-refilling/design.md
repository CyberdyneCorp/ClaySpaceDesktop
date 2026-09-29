## Design

`ClayDocument::place_layer` is the only route that places a whole layer. The
manipulator, the inspector's position and size fields, and a crossing that
puts its result where the source stood all go through it. It now checks the
representation first. For anything that is not a field, it writes the engine's
layer transform, records it on the row, and returns.

The refill did nothing else a carried layer needs:

- **Drawn triangles.** `layout_revision` hashes every carried layer's
  transform. So `mesh_revision` moves, and `App::sync_mesh_layers` rebuilds the
  carried buffer at the new placement.
- **The cache's refusal.** It stops a field from moving to where the bricks
  cannot follow. A carried layer has no bricks, so there is nothing to refuse.
- **Undo.** The engine holds the placement, and `resync_layer_transforms`
  reads it back after an undo or redo step. That path is unchanged.

## Measurement

A headless probe ran the application's per-frame path for a drag:
`set_target_transform`, then `SurfaceGeometry::sync`, then the carried rebuild
and upload through `Renderer::set_mesh_layers`. It ran against main and against
this change, as a release build on Metal. The machine was under heavy parallel
load, so each figure is a range:

- **Mesh reference** (source field drawn under the mesh): 50–57 ms per frame
  before, 11–13 ms after. Both include an 8–15 ms offscreen readback. The
  surface sync went from 44–54 ms to 0 ms.
- **Starting form crossed to a mesh:** sync went from 23–28 ms to 0 ms.
- **Adaptive sheet across the starting form:** sync went from 5–6 ms to 0 ms.
- **Grid beside its source:** edit went from 0.2–1.0 ms to under 0.1 ms.

The regression test `first_mesh_subtool_drag_frame_fits_the_frame_budget`
measured 56.87 ms on main, with 1,049 keys re-meshed, and 4.71 ms with this
change.
