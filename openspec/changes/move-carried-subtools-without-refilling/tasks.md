## 1. Implementation

- [x] 1.1 `place_layer` writes a carried layer's transform without refilling the brick cache.

## 2. Verification

- [x] 2.1 `carried_layer_drag.rs`: move a mesh, a grid, a hierarchy and an adaptive surface over a field. No drag frame dirties a brick, and the drawn triangles follow. A field subtool still refills, and a carried move is still one undo step.
- [x] 2.2 `gizmo_first_drag.rs`: on the mesh reference scene, the first drag frame of a whole mesh subtool re-meshes no brick and takes under 16.7 ms.
- [x] 2.3 Profile the first drag in each representation and record the figures in the proposal.
