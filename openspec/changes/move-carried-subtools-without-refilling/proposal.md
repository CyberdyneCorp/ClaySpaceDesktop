# Move a carried subtool without refilling the field

Issue #196, row D14: the first gizmo drag after `transform/set_mode` stalled
and later drags did not. #300 took the whole SDF layer out of that first frame
with a GPU preview. Measured on main after it, for each representation and
target (release build, headless Metal, reference scenes):

| drag | document edit + surface sync per frame |
|---|---|
| whole SDF layer | deferred until release (#300) |
| placed SDF object | 30–60 ms on the first frame, then deferred by the widget |
| whole mesh subtool beside its source field | 50–57 ms on every frame (1,049 bricks re-meshed) |
| whole mesh subtool, starting form crossed | 23–28 ms on every frame |
| whole adaptive surface across a field | 5–7 ms on every frame |
| whole grid beside its source field | 0.2–1 ms |

## Why

A carried layer is a mesh, grid, hierarchy or adaptive surface, and it holds no
field content. The engine carries its triangles, and the layer transform places
them when the carried buffer is assembled (`append_mesh_layer` and its
siblings). Even so, placing one refilled its box in the brick cache, as a field
layer needs. Wherever a field shared that box, the surface sync re-meshed
bricks that had not changed. That field might be the source a crossing leaves
beside its result, or a form the subtool was moved across. The widget's
adaptive deferral did not catch this. It times only the document edit
(2–6 ms), and the re-mesh ran afterwards in the viewport sync.

## What Changes

- `place_layer` writes a carried layer's transform and returns without dirtying
  the brick cache. The carried buffer still follows, because the layer
  transform is part of the layout revision that triggers its rebuild.
- A field layer is unchanged. Moving it still refills where it was and where it
  goes, and the cache can still refuse a scale it cannot track.

## Impact

On the mesh reference scene, the first drag frame of a whole mesh subtool took
56.9 ms before this change, 47.3 ms of it in the surface sync. It now takes
4.7 ms: 0.4 ms for the edit, no sync, and 4.3 ms to rebuild and upload the
carried buffer. Later frames cost the same. No field brick changes, so the
field is drawn exactly as before.

Not in this change: a placed SDF object still pays one live frame before the
widget defers it. #300 kept that path live on purpose, so that small objects
follow the hand. I14 (`set_grid_display` with no grid layer) is still not
reproduced.
