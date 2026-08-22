## 1. Phase 1 — the capability table

- [x] 1.1 Add a declared per-representation verb table to `clayspace-model`, keyed by `ToolKind` and `Representation`, carrying what each tool maps to on each side
- [x] 1.2 Rewrite `ToolKind::availability` to consult the table, removing the blanket `Unavailable::MeshLayer` at `tools.rs:240`
- [x] 1.3 Add `Unavailable` variants for the reasons that survive: layer protected, layer hidden, prerequisite attribute missing
- [x] 1.4 Add a test asserting the table covers every `ToolKind` on every `Representation` — no tool may be silently absent from the table
- [x] 1.5 Add a test asserting the table's verb count against ClayCore's own enums, so a verb the engine adds and this application has not taken up fails rather than passing quietly

## 2. Phase 1 — the shell follows the layer

- [x] 2.1 Carry the active layer's representation in `ShellState`
- [x] 2.2 Show the representation in the viewport bar, distinguishable by more than colour
- [x] 2.3 Show the representation beside each layer in the layer stack
- [x] 2.4 Make the brush shelf list only the tools the active representation has
- [x] 2.5 Keep the active tool across a layer change where the new representation has it; substitute and state the substitution where it does not
- [x] 2.6 Hold brush settings per tool *and* per representation
- [x] 2.7 Localise the new strings in all three locales
- [x] 2.8 Regenerate the shell captures and check the shelf reads correctly for each representation
- [x] 2.9 Update `docs/features.md` and `README.md` for what the shelf now does

## 3. Phase 2 — conversion in the engine bridge

- [x] 3.1 Wrap `clay_voxel_rasterize` in `claycore`, with the region and cell size
- [x] 3.2 Wrap `clay_voxel_rasterize_mesh` for the direct mesh-to-voxel path
- [x] 3.3 Wrap `clay_item_volume_from_voxels` and `clay_voxel_to_layer`
- [x] 3.4 Add a `ClayDocument::convert_layer` that produces a new layer and leaves the source untouched
- [x] 3.5 Add a cost estimate — surface movement, vanishing feature size, grid size against the memory budget — computed from the chosen cell size
- [x] 3.6 Refuse an unbounded region and an unaffordable resolution, each with its own reason
- [x] 3.7 Test that a conversion is not undoable and that removing its layer takes it back — a conversion produces no engine undo entry at all, so the spec says what is true and `conversion.rs` holds the measurement
- [x] 3.8 Test that a coloured voxel sculpt keeps its colours across voxel-to-SDF
- [x] 3.9 Test that mesh-to-voxel keeps a feature thinner than a cell that the SDF detour loses

## 4. Phase 2 — conversion in the interface

- [x] 4.1 Add the conversion commands to `Command` and route them through the existing dispatch
- [x] 4.2 Add a conversion panel showing direction, cell size, region, the computed costs, and that the crossing is not undoable
- [x] 4.3 Recompute the stated costs as the resolution changes
- [x] 4.4 Put the conversion behind the busy cursor, since it is unbounded work
- [x] 4.5 Localise the panel in all three locales
- [x] 4.6 Capture the panel for each direction
- [x] 4.7 Document the round trip in `docs/features.md`

## 5. Phase 3 — mesh sculpting

- [x] 5.1 Wrap `clay_mesh_sculptor_create` / `destroy` / `refresh` / `refit` / `quality` in `claycore`
- [x] 5.2 Wrap `clay_mesh_sculptor_stamp` and `clay_mesh_sculptor_apply_stroke`
- [x] 5.3 Hold a mesh sculptor per mesh layer in `ClayDocument`, created on first sculpt and invalidated when the geometry is replaced
- [x] 5.4 Route a stroke on an active mesh layer to the mesh sculptor
- [x] 5.5 Add the sixteen mesh brushes to the capability table and the shelf
- [x] 5.6 Make mesh layers pickable, so a press sculpts rather than orbits
- [x] 5.7 Record a mesh gesture as one undoable action that reverts bit-exactly
- [x] 5.8 Report the mesh quality figure and name retopology as the remedy when a stroke passes it
- [x] 5.9 Test that indices and quads are byte-identical across a stroke
- [x] 5.10 Wrap and expose paint and smear, refusing a mesh with no colour attribute with a stated reason
- [ ] 5.11 Wrap and expose `clay_mesh_sculptor_deform` for taper and twist as layer operations
- [ ] 5.12 Wrap and expose the mesh lattice cage
- [ ] 5.13 Visual captures for the mesh brushes, before and after

## 6. Phase 3 — voxel vocabulary

- [ ] 6.1 Wrap and expose the voxel verbs not yet reached: carve-with-alpha, flood select, box fill, line fill
- [ ] 6.2 Wrap and expose the cube and sphere paint/erase brushes with their falloff curves
- [ ] 6.3 Wrap and expose pre-bake repair: report, close holes, fill voids
- [ ] 6.4 Add a repair panel that reports before it changes anything
- [ ] 6.5 Expose regional refinement through `clay_voxel_add_level_region`
- [ ] 6.6 Test that a repair's report changes after the repair it describes

## 7. Phase 3 — voxel sculpt layers

- [ ] 7.1 Wrap the `clay_voxel_sculpt_layer_*` family in `claycore`
- [ ] 7.2 Add begin/end recording, with the recording state visible in the shell
- [ ] 7.3 Present the sculpt layer stack inside the existing layer panel, nested under the voxel layer it belongs to: show, hide, reorder, merge down, remove
- [ ] 7.4 Make strength adjustable after recording, and separately undoable from the strokes
- [ ] 7.5 Report per-layer and total memory cost
- [ ] 7.6 Carry sculpt layers through save and reload, or refuse the save with a stated reason if the format cannot hold them
- [ ] 7.7 Test that strength survives a save and reload

## 8. Phase 3 — SDF vocabulary

- [ ] 8.1 Expose the combine operations in the options bar where an edit's op is chosen
- [ ] 8.2 Expose the five blend profiles beside them
- [ ] 8.3 Add a PNG alpha decoder to `clayspace-engine` beside `import_mesh`, promoting `png` from a dev-dependency, and refuse a file that is not a PNG with a stated reason
- [ ] 8.4 Wrap `clay_item_add_alpha` and `clay_voxel_sculpt_carve_alpha`, and add an alpha source to the brush
- [ ] 8.5 State where an alpha is not accepted rather than offering a dead control
- [ ] 8.6 Expose the deformers as layer operations with their parameters, each one undo step
- [ ] 8.7 Wrap `clay_item_set_gate` so a mask gates a combine operation, not only a brush
- [ ] 8.8 Test that a masked region survives a subtracting edit that crosses it
- [ ] 8.9 Visual captures for the combine operations and the blend profiles

## 9. Close-out

- [ ] 9.1 Re-record the performance baseline, since the tool set and the stroke routing have both changed
- [ ] 9.2 Update `docs/features.md`'s *Deliberately absent* and *Not built yet* sections
- [ ] 9.3 Update `README.md`'s input table and feature summary
- [ ] 9.4 Run the full gate and record the test count
