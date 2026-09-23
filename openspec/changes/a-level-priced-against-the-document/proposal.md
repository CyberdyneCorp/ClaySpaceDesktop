## Why

An audit of the mesh and subdivision-hierarchy path measured three defects (#184).

**A retopology, and a hierarchy built from one, were drawn unlit.** `clay_mesh_from_triangles` takes positions and indices and nothing else, so the layer a retopology lands in holds no normals. A hierarchy exports a level's normals only where its cage carried its own (`export_wants` in the engine's `multires_eval.cpp`), so a hierarchy over that cage inherits the absence at every level. Both readers stood a single `+y` in for the missing normals, which lights the whole form as one colour. Measured on the starting form, the drawn normals sat 88.8 degrees off the triangles they lit.

**A merge down of the bottom pass surfaced a raw engine result.** Every other refusal on a hierarchy's stack is a sentence. The bottom pass has nothing beneath it — the form under the passes is not a pass — and the engine's own refusal code was passed through as it came.

**A level was priced against nothing but itself.** `SubdivisionCost::within` compared the engine's preflight peak with the budget, and the preflight is arithmetic on the level below: it prices the new level alone. A fifth level was admitted with the document already holding most of what a budget allows, and the report read 765 MB afterwards with a 5.1 s freeze. The same audit read a 1,240-quad retopology quoting 7,440 faces at level one as a pricing error; it is not one. The layer holds the retopology as its triangulation, 2,480 triangles, and a triangle's first Catmull-Clark step makes three quads.

## What Changes

- **Normals are derived where the engine hands back none.** `claycore::Mesh::normals_or_derived` answers the mesh's own normals where it carries them and area-weighted normals from its triangles where it does not. The mesh-layer reader and the hierarchy's level mesh both use it, so a retopology and every level of a hierarchy over it are lit by their own shape. This is derived shading data for a surface the engine produced, not a surface the viewport reconstructs.
- **A merge down of the bottom pass is refused with a sentence** that names what the pass can do instead (`Fundir na forma`). `MultiresState::nothing_beneath` answers the question from the stack's order.
- **A level is priced on top of what the document holds.** `SubdivisionCost::within(held_bytes, budget_bytes)` refuses when the document's ledger plus the level's peak exceeds the budget, and `Refusal::LevelOverBudget` names all three figures. The document reads its own ledger (`ClayDocument::memory`) once, before the hierarchy is asked for a level.
- **A triangle cage's arithmetic is stated.** `SubdivisionCost::faces_after_triangles` projects three faces per triangle for the first step and four after it, and the engine's quote is held against it.

## Capabilities

### Modified Capabilities
- `representation-modes`: a level's refusal names what the document holds, and a merge down of the bottom pass is refused with a sentence.
- `representation-conversion`: subdividing is priced on top of what the document already holds, and the faces quoted from a triangle cage follow the three-then-four rule.
- `viewport-rendering`: a surface the engine hands back without normals is lit by its own shape.

## Impact

**Code**: `claycore` (`mesh.rs`, `document.rs`), `clayspace-model` (`multires.rs`, `conversion.rs`), `clayspace-engine` (`multires.rs`, `document.rs`).

**Not in this change**: moving a subdivision off the interface thread. The audit's 5.1 s freeze is shared with the UI-thread issue, which owns the worker path; this change stops the level that did not fit from being built at all, and does not change how long one that fits takes. The budget itself is unchanged at `LEVEL_BUDGET` (2 GB): the 512 MB the audit compared against is the brick cache's own budget, which `state` reports beside the whole document's ledger and which does not bound a hierarchy.
