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
- [x] 5.4 Route a stroke on an active mesh layer to the mesh sculptor — the model half; the viewport half is 5.14
- [x] 5.5 Add the sixteen mesh brushes to the capability table and the shelf
- [x] 5.6 Make mesh layers pickable, so a press sculpts rather than orbits
- [x] 5.7 Record a mesh gesture as one undoable action that reverts bit-exactly
- [x] 5.8 Report the mesh quality figure and name retopology as the remedy when a stroke passes it
- [x] 5.9 Test that indices and quads are byte-identical across a stroke
- [x] 5.10 Wrap and expose paint and smear, refusing a mesh with no colour attribute with a stated reason
- [x] 5.11 Wrap and expose `clay_mesh_sculptor_deform` for taper and twist as layer operations
- [x] 5.12 Wrap and expose the mesh lattice cage
- [x] 5.13 Visual captures for the mesh brushes, before and after
- [x] 5.14 Draw mesh layers in the viewport: a second geometry source beside `SurfaceGeometry`, since a mesh layer has no bricks. This is the other half of 5.4's "the viewport shows the result"

## 6. Phase 3 — voxel vocabulary

- [!] 6.1 Wrap and expose the voxel verbs not yet reached — **partly blocked, and split**:
  - carve-with-alpha needs the PNG decoder, which is task 8.3 in group 8. Do it there, with the SDF alphas, rather than half here
  - flood select needs a *selection* — a set of cells the interface can hold, show and act on — and this application has no such concept. That is its own design, not a wrapper
  - box fill and line fill write cells directly from two points. `fill_box` and `fill_line` are wrapped in `claycore` already; what they lack is a gesture, and a stroke's endpoints are a poor stand-in for a box a user expects to see while dragging it
- [x] 6.2 Wrap and expose the cube and sphere paint/erase brushes with their falloff curves
- [x] 6.3 Wrap and expose pre-bake repair: report, close holes, fill voids
- [x] 6.4 Add a repair panel that reports before it changes anything
- [x] 6.5 Expose regional refinement through `clay_voxel_add_level_region`
- [x] 6.6 Test that a repair's report changes after the repair it describes

## 7. Phase 3 — voxel sculpt layers

- [x] 7.1 Wrap the `clay_voxel_sculpt_layer_*` family in `claycore`
- [x] 7.2 Add begin/end recording, with the recording state visible in the shell
- [x] 7.3 Present the sculpt layer stack inside the existing layer panel, nested under the voxel layer it belongs to: show, hide, reorder, merge down, remove
- [x] 7.4 Make strength adjustable after recording, and separately undoable from the strokes
- [x] 7.5 Report per-layer and total memory cost
- [x] 7.6 Carry sculpt layers through save and reload, or refuse the save with a stated reason if the format cannot hold them
- [x] 7.7 Test that strength survives a save and reload

## 8. Phase 3 — SDF vocabulary

- [x] 8.1 Expose the combine operations in the options bar where an edit's op is chosen
- [x] 8.2 Expose the five blend profiles beside them
- [x] 8.3 Add a PNG alpha decoder to `clayspace-engine` beside `import_mesh`, promoting `png` from a dev-dependency, and refuse a file that is not a PNG with a stated reason
- [x] 8.4 Wrap `clay_item_add_alpha` and `clay_voxel_sculpt_carve_alpha`, and add an alpha source to the brush
  - Voxels and meshes take a stamp and are measured doing it. An SDF *stroke*
    does not: `clay_layer_apply_stroke` uses its item as a template and does not
    carry the deformer chain hung off it, measured at three amplitudes under two
    ops. The wrapper works on a placed item and is tested there. Filed in
    `docs/roadmap.md`; `claycore/tests/alpha_deformer.rs` is the tripwire.
- [x] 8.5 State where an alpha is not accepted rather than offering a dead control
- [x] 8.6 Expose the deformers as layer operations with their parameters, each one undo step
- [x] 8.7 Wrap `clay_item_set_gate` so a mask gates a combine operation, not only a brush
  - Wrapped and matching the documented contract. The engine accepts the call
    and does nothing: measured with a mask sampling 1.0 at the cut's own centre
    and 65,752 cells painted, at every width and threshold tried, never
    refusing. The application does not call it — a call per stroke that does
    nothing is a cost with no benefit and a promise the interface could not
    keep. Filed in `docs/roadmap.md`.
- [x] 8.8 Test that a masked region survives a subtracting edit that crosses it
  - Written the other way round, because that is what is true: the tests hold
    that a mask gates authoring and not the operation, and fail the day it
    does. `claycore/tests/mask_gate.rs` measures it at the engine boundary and
    `clayspace-engine/tests/mask_gate.rs` through the domain. An `#[ignore]`d
    aspiration would have recorded nothing.
- [x] 8.9 Visual captures for the combine operations and the blend profiles

## 9. Close-out

- [x] 9.1 Re-record the performance baseline, since the tool set and the stroke routing have both changed
- [x] 9.2 Update `docs/features.md`'s *Deliberately absent* and *Not built yet* sections
- [x] 9.3 Update `README.md`'s input table and feature summary
- [x] 9.4 Run the full gate and record the test count
  - `just check` green: formatting, layering, clippy with `-D warnings`, the
    whole suite, `openspec validate --all --strict`, and the packaging tools.
    **687 tests** across 86 binaries and **240 visual captures**, up from 620
    and 177 when this change opened. The performance gate is per-platform now —
    `benchmarks/baseline-linux-x86_64.json` recorded against ClayCore 0.39.0
    with the three representations in place, dab median 2.42 ms against a 50 ms
    budget. The macOS baseline still reads 0.29.1 and needs a run on that
    hardware.

## 10. Found while checking the work

Two gaps the task list did not name, both found by asking a question none of
the existing tests asked.

- [x] 10.1 Rename and delete a layer from the interface
  - The model has carried `rename_layer` and `remove_layer` from the start and
    the layer panel offered neither. Both are on a row's own menu now, and a
    rename also on a double-click; the field opens in place and a refusal
    leaves what was typed in it. **Excluir** is disabled with its reason for the
    last layer. Two defects turned up on the way and are held by
    `visual_shell.rs`: a row sensed clicks only across the width of its own
    text, so a short name left most of the row dead; and a truncated label
    registers a hover widget, so a row's interaction had to be claimed *after*
    the label or long names were shadowed by it.
- [x] 10.2 Make a voxel sculpt reach the screen
  - `voxel_tools.rs` and `visual_sculpting.rs` both asked the grid whether it
    had changed. It always had, and nothing was drawn: the viewport builds its
    surface from the brick cache, the cache holds the document's SDF field, and
    the engine states that a voxel layer carries no SDF content. A document
    holding one sculpted grid meshed to **zero triangles**. Two more parts of
    the chain had the same assumption — `bounds` answered `None` so Enquadrar
    tudo framed a default box, and `pick` marched the field so a press orbited
    instead of sculpting. A grid is now drawn through the mesh-layer path with
    `clay_voxel_mesh_smooth`, reports its own extent, and is picked with
    `clay_voxel_raycast`. `clayspace-app/tests/visual_voxel_sculpt.rs` asks the
    sculptor's question: did it appear, did a second stroke move it, can the
    pointer find it.

- [x] 10.3 Count what is on screen, not only what the brick cache built
  - The polygon and vertex counters were fed by `record_geometry`, which only
    the surface cache calls. A mesh or voxel layer draws triangles it knows
    nothing about, so a document whose only layer was a sculpted grid reported
    "Triângulos 0" over a visible sculpt and a detail line saying nothing had
    been meshed. The two sources are summed now, and the "nothing yet"
    classification is made on the total.
- [x] 10.4 Mesh a voxel layer incrementally
  - Drawing it whole was the first attempt and it does not survive
    measurement. On a 0.01 grid a 3.2 ms dab cost **309 ms** to re-mesh,
    against a 50 ms budget and rising with the sculpt — drawn and unusable.
    `clay_voxel_take_dirty_chunks` reports what an edit dirtied and
    `clay_voxel_mesh_chunks` meshes only those: 3.3 ms, flat in the sculpt's
    size. Held by a count of chunks rather than a duration, because a
    millisecond budget on a shared machine measures the machine.
  - The same measurement retired the rounded preview. `clay_voxel_mesh_smooth`
    carries **no vertex normals** — it renders as a flat white silhouette with
    no form to read, captured and compared before deciding — and it has no
    chunked variant. A grid is drawn as the boxes it is; the rounded surface is
    the voxel-to-SDF conversion, which is what that direction is for.

## 11. Crossing into a mesh

The gap the change's own premise left open: `Direction` had four entries and
none of them ended in `Mesh`, so "the three representations are first-class"
was true of two of them and of the third only if you had a file to import.

- [x] 11.1 Add `SdfToMesh` and `VoxelToMesh`
  - Everything needed was already wrapped — `Document::mesh`,
    `Document::attach_mesh_layer`, `VoxelField::mesh`. Nothing was missing from
    ClayCore; the gap was ours.
  - The engine meshes a *document*, not a layer: `clay_document_mesh` takes no
    layer id and there is no layer-scoped mesher. The SDF crossing hides the
    other SDF layers across the call and puts them back, which is exact by the
    engine's own contract and measured — the starting sphere alone meshes to
    57,650 vertices bounded at ±1, the same document with a blob on a second
    layer to 44,462 bounded past 1.3, and the restore gives the first answer
    back. Voxel and mesh layers are left alone: neither carries SDF content.
  - Marching tetrahedra for the field, because what comes out is going to be
    sculpted and exported and that is the watertight, 2-manifold one. The
    greedy mesh for a grid, because the rounded voxel mesher carries no vertex
    normals and what came out would render as a flat silhouette.
- [x] 11.2 State the loss that belongs to these two
  - `Cost::fixed_topology`, and a line in the panel: the topology is the
    sampling lattice's and nothing here re-flows it. Dense, uniform, no edge
    loop following anything — the input a retopology pass replaces rather than
    the output one produces. Said before the crossing, not discovered after it.
- [x] 11.3 Make a mesh layer reachable by the pointer
  - Found while testing 11.1 and the reason "convert to mesh and sculpt" would
    still not have worked. A pick against a mesh layer is answered by the mesh
    sculptor's raycast, which refused until the sculptor was built — which only
    a stroke did. The interface sends no stroke where the pick found nothing,
    so the first stroke could never arrive: **a mesh layer was unsculptable
    through the pointer, imported or converted.** The sculptor is armed when
    the layer becomes active. `to_mesh.rs` holds it in the order the interface
    goes in — select, point, then stroke.
  - Two tests in `mesh_sculpting.rs` had written the deadlock down as
    deliberate — "a pick before any stroke finds nothing, deliberately" and
    "there is no sculptor before the first stroke, so there is no figure". Both
    now assert the opposite and say why the old reading was wrong. The quality
    readout gains from it too: a sculptor deciding whether a mesh needs
    retopology wants the figure before they start, not after.
