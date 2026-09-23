## 1. Normals where the engine hands back none

- [x] 1.1 Add `claycore::area_weighted_normals` and `Mesh::normals_or_derived`, with a unit normal for a vertex no triangle reaches.
- [x] 1.2 Read a mesh layer's normals through it in `Document::read_mesh_layer`.
- [x] 1.3 Read a hierarchy level's normals through it in `Hierarchy::level_mesh`.
- [x] 1.4 Hold both: `a_retopology_and_a_hierarchy_built_from_it_are_lit` (engine, through the document) and `a_hierarchy_over_a_cage_without_normals_is_drawn_with_them` (engine, on a tilted cage the constant would miss by 63 degrees).

## 2. The bottom pass

- [x] 2.1 Add `MultiresState::nothing_beneath`, answered from the stack's order rather than from `index`.
- [x] 2.2 Refuse `MergeDown` on the bottom pass in `Hierarchy::apply_sculpt_layer_op` with a sentence naming `Fundir na forma`.
- [x] 2.3 `merging_the_bottom_pass_is_refused` (engine) and `only_the_bottom_pass_has_nothing_to_merge_into` (model).

## 3. The price of a level

- [x] 3.1 `SubdivisionCost::within(held_bytes, budget_bytes)`, saturating, and `Refusal::LevelOverBudget { held_bytes, .. }` naming all three figures.
- [x] 3.2 Read the document's ledger in `apply_multires_level_op` before the hierarchy is borrowed, and pass it to `Hierarchy::add_level`.
- [x] 3.3 `SubdivisionCost::faces_after_triangles` for a triangle cage's first step.
- [x] 3.4 `a_level_is_priced_against_what_the_document_holds` (model), `a_level_over_budget_is_refused` and `a_level_is_quoted_from_the_faces_the_cage_holds` (engine).
