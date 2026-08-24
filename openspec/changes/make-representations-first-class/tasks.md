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

## 12. The polyframe, and what Move actually does

- [x] 12.1 Draw a mesh layer's own edges over it
  - ZBrush's PolyF, on **Visualizar → Malha aparente** and Shift+F (F alone is
    already framing, so the shifted pair keeps both where a ZBrush hand expects
    them). A line list over the mesh layers' own vertex buffer — an index
    buffer rather than a second mesh, because the positions are already
    uploaded — with a depth bias so the lines sit in front of the triangles
    they outline instead of fighting them.
  - Edges are deduplicated. Not to halve the buffer: the lines are translucent,
    so an edge shared by two triangles and emitted twice is blended twice and
    the interior reads heavier than the silhouette.
- [x] 12.2 Establish whether Move and Move Topological reach a mesh
  - **Move: yes, and always was** — the capability table binds it to
    `clay_mesh_sculptor_stamp (GRAB)`. What it was not was *reachable*, for the
    reason in 11.3.
  - **Move Topological: yes, and it is the only kind there is.** The engine's
    mesh brush descriptor carries `geodesic` and defaults it on; this
    application sets it for every verb but Planar and Raspar. Measured on a
    horseshoe whose tips are 0.71 apart through the air and 2.36 around the
    arc: a brush reaching 1.0 drags one and leaves the other.
    `clay_item_volume_move_topological` is a different call — it takes an item
    carrying a volume — and belongs to the SDF side.
- [x] 12.3 Make the brush's size and intensity reach a mesh stroke
  - Found while measuring 12.2, and a real defect. The engine states that
    `clay_mesh_sculptor_apply_stroke` **ignores the descriptor's radius and
    strength** and takes each stamp's from the preset; the mesh path built its
    own preset carrying only spacing, so every mesh stroke ran at the engine's
    default radius of 0.25 whatever the brush said — sizes 0.1, 0.5 and 1.0 all
    moved the same 944 vertices, and Intensidade was inert the same way.
  - The same line had spacing inverted against every other path: the design
    reads flow as "more flow, stamps closer together" and the SDF path spells
    that `1.0 - flow`, while this passed it through raw. On a dragging verb
    that decides whether a second stamp is emitted at all, and a stroke of one
    stamp has no motion to drag by — which is why Move looked broken rather
    than merely coarse. Both fixed by using the shared preset the SDF path
    already had.

## 13. Measured against Blender, and what that found

Driven over the Blender MCP addon on a matched sphere: same brush radius in
world units (`unprojected_size`), same strength, same stroke, and the same
metric computed on both sides — the mean angle between adjacent vertex normals
before and after, which reads as how much a verb shredded the surface.
**Blender scored 1.00x on every brush.** Ours did not.

- [x] 13.1 Stop a mesh stroke building on itself
  - Inflar 5.04x, Pinçar 9.41x, Vinco 3.71x. Padrão, the control, 1.11x —
    it displaces along the *region's* averaged normal, so nothing compounds.
    The rest displace along each vertex's own normal or gather toward a centre,
    and a building stroke feeds each stamp back into the normals the next one
    reads. Clamped on the mesh path now; ratios fall to 1.18x, 1.83x, 1.34x.
  - Scoped to that path rather than to `Shaping::default`, and that was a
    correction: changing the domain default broke four `masking.rs` tests, and
    the evidence for the change was entirely from mesh verbs. It is a fact
    about those verbs, not about brushes — the same reason `MAX_JITTER` lives
    beside the preset. The field and the grid are untouched.
- [x] 13.2 Give Nudge a direction
  - `clay_mesh_sculptor_apply_stroke` derives a drag direction for GRAB and
    SNAKEHOOK and for nothing else, so NUDGE — which projects the drag into
    each vertex's tangent plane — was handed the descriptor's default of all
    zeroes. It moved **not one vertex** at any size, intensity or stroke
    length; Blender's moved 5% of the mesh on the same stroke. It also ignored
    Intensidade, which the engine never applies to this verb, so the slider now
    reaches it through the vector.
  - The magnitude is a calibration and is labelled one: ours is rougher than
    Blender's at any given displacement, which is the engine's tangent-plane
    push rather than something a constant fixes. `NUDGE_PUSH` carries the
    measurements.
- [x] 13.3 Take the engine's brush defaults
  - `MeshStamp::as_raw` built the descriptor from a **zeroed** struct, so every
    field the type does not name was 0 — and those are not harmlessly zero.
    `polish_angle: 0` is a fully closed gate, so Polir smoothed nothing even
    across a crease cut for it; `layer_height: 0` is a zero ceiling, so Camada
    moved 0.0086 against Padrão's 0.678. `clay_mesh_brush_defaults` exists for
    exactly this — "so a host fills in what it means and takes the rest" — and
    we were taking nothing.
- [x] 13.4 Hold it with a measurement, not a picture
  - `visual_mesh_verbs.rs`: no verb may exceed 2.0x roughness, and every verb
    the shelf offers must move something. Captures beside them.
- [x] 13.5 Make the mask test measure masking
  - It asserted `held < raised` over a mask 0.2 wide under a brush of radius
    0.3 — the brush reached across the strip from both sides, so the two
    differed by 7e-7 on a 0.14 deposit and it passed on rounding. The mask is
    wider than the brush now, and asked properly the answer is emphatic:
    1.0005 against an unmasked 1.1400, on a sphere that started at 1.0.
  - An intermediate reading said a building stroke defeats masking. It does
    not — that measurement was taken against a polluted fixture and is wrong.
    The protection above is with accumulation on.

- [x] 13.6 Hold a mesh drag as one gesture
  - The first thing found and the last thing fixed. The ViewModel applies a
    stroke in segments as the pointer travels, which is what keeps a field
    live under the pointer — but Grab anchors on its first stamp, so segments
    are several grabs, each anchoring where the last stopped. Against Blender's
    Grab on a matched sphere with the same drag: one call reaches 9.8% and
    moves 0.707, Blender 11.4% and 0.779, two segments 19.0% and 0.569. Two
    anchors sharing one drag, which is the crease along the path.
  - Ruled out on the way: the geodesic falloff, which makes no difference to
    Grab at all — measured identical either way. Blender's Grab and ZBrush's
    plain Move are Euclidean, but that is not what was wrong here.
  - A mesh drag now shows nothing until the pointer comes up. Reverting each
    segment before re-applying the gesture from its anchor would buy the
    preview back and needs the engine to hold a gesture open across calls.

- [x] 13.7 Make a drag pull rather than slide
  - Reported as "it seems we're sliding the faces, instead of actually
    moving", which was exact. The interface picked the surface under the
    pointer at **every** sample, so every position landed on the form and the
    motion between two of them was a walk along it — the skin stretched and
    folded and nothing was carried anywhere. A drag across the silhouette lost
    half its samples outright: ten of twenty-one.
  - A dragging verb now takes hold once and follows the pointer at the anchor's
    depth (`input::dragged_to`, with its own tests). And Mover is applied as
    one stamp at the anchor rather than a resolved stroke: a stroke walks the
    brush centre along the path, so a drag leaving the surface takes the centre
    with it and the later stamps reach nothing — measured, a dent where a lobe
    should have been.
  - The drag is scaled by the intensity, because the descriptor's `strength`
    weights the falloff and not the displacement. Blender's Grab carries its
    region by drag × strength; matched, ours moves its furthest vertex 1.128
    against Blender's 1.129 on the same gesture.

- [x] 13.8 Clear a removed layer out of the cache
  - Reported as the starting form sitting under the sculpt and never changing,
    with the real result only after a save and a reopen. The brick cache holds
    the evaluated field, and the removal marked the *remaining* active layer
    dirty — which cannot help, because the stale bricks belong to the layer
    that left. Measured: the removed sphere still meshed to the same 298,680
    triangles through an incremental sync and a full rebuild alike, and still
    answered a raycast at [0, 0, 1]; 17,160 and no hit once its own region is
    re-evaluated. Hiding was always right and is held by the same test.

- [x] 13.9 Upload a mesh layer the moment it exists
  - Reported as everything disappearing when the SDF layer was deleted, and
    coming back the moment sculpting began. `mesh_revision` is what the
    viewport watches to decide whether to upload the carried layers, and
    adding a mesh layer moves no vertex and touches no grid — so it did not
    change and the layer was never uploaded. A crossing only *looked* right
    because the source layer was still contributing to the field: the sphere
    on screen was the field. Remove the source and there is nothing, with
    62,576 vertices sitting unuploaded; make a stroke and the old number moves
    and they appear. Which layers are carried, and whether each is shown, are
    part of the number now.

- [x] 13.10 Show a mesh drag while it happens
  - The cost 13.6 accepted, now paid off. Segments are back, and each replays
    the gesture from its anchor rather than carrying only what is new — the
    model holds the previous segment's vertex deltas, reverts them, and lays
    the whole gesture down again. `begin_gesture`/`end_gesture` on `SculptModel`
    say when a gesture is open; only the release banks anything, so one drag is
    one undo however many segments drew it, and a cancelled gesture takes its
    preview with it.
  - The number the viewport watches is bumped by every preview, because a
    preview banks nothing and would otherwise leave it sitting still while the
    surface was visibly moving.
  - And a replayed segment fires on every pointer move rather than after three
    stamps' worth of travel, which is the threshold a *stamping* segment needs
    and the reason the first attempt still showed nothing: at the default flow
    and a brush of 0.858 that threshold is 1.03 world units, most of the way
    across a unit sphere. Measured, 40 pointer moves now produce 40 updates
    where they produced none.
  - It costs about 17 ms a move on a 140,774-vertex mesh, nearly all of it the
    stamp rather than the buffer it fills (1.2 ms). Dropping the surface walk
    takes it to 12.8 and is not taken: with the single-stamp path the walk is
    what makes Move topological, and the horseshoe test fails without it. That
    measurement also corrects an earlier note that the walk makes no difference
    to Grab — true of the resolved-stroke path, not of this one.

- [x] 13.11 Make Suavizar smooth, and be seen doing it
  - Reported as no effect at all. Three causes, none of them the same.
  - The mesh clamp from 13.1 applies to every verb, including the ones that
    *converge*. A smoothing verb averages toward the neighbourhood, so running
    it again moves less each time and it cannot shred — clamping one means
    never smoothing more than a single stamp's worth however long a sculptor
    rubs. Suavizar, Relaxar and Polir are exempt now.
  - The engine's SMOOTH is a one-ring Laplacian, a high-frequency filter that
    barely touches a bump spanning many edges, and `smooth_iterations` was
    never set so it ran at the engine's default. Measured on a ridge 0.0676
    proud of a unit sphere, four passes: 1.0670 at the default and clamped,
    1.0187 at sixty-four passes and accumulating. Cheap — 5.4 ms against 4.0 ms
    for a 0.18 brush on 140,774 vertices.
  - And it is *region-based*, so it was held until the pointer came up. That is
    right on a field, where it bakes a region and puts it back, and wrong on a
    mesh where it is an ordinary stamp. Mesh segments also fire every one stamp
    rather than every three, since nothing is re-meshed: measured, a forty-move
    drag now sends eleven updates where it sent none.
  - An intermediate reading said accumulation made no difference to Suavizar.
    It was measured against a build that forced the clamp regardless of the
    brush, so the flag never reached the engine.

**Still open**: Move reaches 8.3% of the sphere against Blender's 0.9% for the
same nominal radius, which is a units question and not a defect found. Camada
is better but weak (0.0086 against Blender's 0.341).
Pintar and Borrar do nothing on a colourless mesh and Pintar reports success
while doing it. Move covers 27% of the sphere to Blender's 1.8% at the same
nominal radius, which is a units question rather than a defect found.

## 14. The keys a sculptor holds

- [x] 14.1 Hold Shift to smooth, hold Ctrl to invert
  - Both references let a sculptor reach the two most-used alternatives without
    leaving the tool in hand. The keys are read at the press and held for the
    gesture — a key caught mid-drag would change the verb under the sculptor's
    hand, and neither reference does that — and the shelf never moves, so
    letting go returns to the selected tool without re-picking it.
  - **Ctrl and not Alt**, which is how ZBrush spells it: Alt already forces the
    drag to orbit, which is ZBrush's own rule and the one that leaves a
    trackpad with no second button able to turn the model, and while rigging it
    means "move this sphere". Blender spells invert Ctrl and Ctrl is free here
    during a stroke. Shift was unclaimed either way.
  - Inverting is three different mechanisms because the three representations
    are: a field turns the combine operation over (Add/Subtract,
    Emboss/Engrave, Relief/Incise, and `None` for an operation with no
    opposite, which is left alone rather than quietly becoming another verb); a
    mesh negates the brush strength; a grid erases, because occupancy is binary
    and there is no sign to turn.
  - The first attempt negated the *stroke preset's* strength on the mesh, and
    measured as no change at all: the preset's strength is contracted to
    `[0, 1]` and the resolver drops any stamp whose strength is not positive.
    The descriptor's strength is the signed one, and a resolved stroke
    multiplies the two — so that is where the sign lives. Measured on a unit
    sphere, the same sweep reaches 1.054 upright and 0.945 held, which is
    symmetric to within a thousandth.
  - The header's note that a stroke IGNORES the descriptor's radius and
    strength is half right: the radius is replaced per stamp, the strength is
    multiplied.
  - Whether the following samples come from a plane or from a fresh surface
    pick is decided by the *substituted* tool. Reading the shelf would carry a
    Shift-held smooth across a drag plane it never touches.

## 15. Masking, on the key and on the screen

- [x] 15.1 Put mask painting on `M`, as a toggle
  - Blender's sculpt mode spells masking `M`, and it is the key a hand coming
    from there reaches for. It held the material cycle; that moved to `Shift+M`
    rather than the mask taking a modifier, because freezing a region is done
    constantly while sculpting and changing the display material is done once.
  - A toggle rather than a plain selection: freezing is a detour from what is
    being sculpted, and the way back should be the same key rather than a hunt
    across the shelf. Choosing a tool outright ends the detour, so the next `M`
    starts a fresh one instead of rewinding past the choice. The toggle goes
    through the same selection every other tool does, so Máscara keeps its own
    remembered brush and hands the sculpting brush back unchanged.

- [x] 15.2 Draw the frozen region
  - It was invisible. The engine has had `clay_mask_sample` all along and the
    viewport had no idea the mask existed — which is worse than no mask:
    masking protects almost completely, so a sculptor who freezes a region and
    finds a brush doing nothing cannot tell a protected surface from a broken
    tool.
  - A vertex attribute of its own rather than a darkened vertex colour. Colour
    modulates the material and is gated on `material.tint.a` — which is zero
    for a mesh carrying no vertex colours, meaning every SDF surface — so a
    mask ridden in on it would have been discarded in the shader.
  - It needs its own upload pass. A mask stroke moves no clay and dirties no
    brick, deliberately, so the incremental re-mesh has nothing to re-mesh and
    would leave what was just painted undrawn. `SurfaceGeometry::refresh_mask`
    re-samples every stored vertex, driven by a `mask_revision` counter the way
    the carried layers are driven by `mesh_revision`. An ordinary stroke must
    *not* move that counter, or every dab would pay for a full re-sample; a
    test holds that.
  - A re-mesh replaces a brick's vertices outright, so newly meshed vertices
    are sampled too — the dirty subset only, which is where the cost belongs.

- [x] 15.3 Make the mask tool mean the same thing on all three
  - Found by the visual test refusing to run. `apply_stroke` asked the
    representation first, and two of the three arms got the mask wrong: on a
    grid Máscara fell through to the depositing arm and **added clay** where
    the sculptor asked to freeze a region (measured: 426 indices of new
    material into an empty grid, and no mask afterwards), and on a mesh the
    tool table gave it no verb at all though `stroke_mesh` had been passing the
    mask to the engine the whole time.
  - The mask arm is hoisted above the representation match now, because a mask
    is not part of any representation: it is a world-addressed field the verbs
    consult. Three tests that counted the mesh shelf against the engine's
    sixteen fixed-topology brushes count Máscara apart rather than being
    loosened, so a real seventeenth brush would still be caught.

## 16. The Máscaras menu, entry by entry

- [x] 16.1 Give the operations an amount the interface can set
  - Three of the six took one and nothing could change it: the menu dispatched
    `Expand(1)`, `Contract(1)` and `Smooth(1)`, so expanding a mask by four
    cells meant clicking four times. An extrusion was worse — `ExtrudeSettings`
    lived in the ViewModel and no command could write to it, so thickness,
    rim rounding and rim smoothing were unreachable and every wall the
    application could build was 0.08 thick with a hard edge.
  - A **MÁSCARA** section of the inspector holds them, shown once a mask exists
    — which is also when every operation but Limpar becomes usable. The menu
    spells the amount out beside each entry, because the same entry now does a
    different amount of work depending on the panel, and the two units it
    stands for (cells and passes) are not the same quantity.
  - The amount is filled in by the ViewModel rather than the View, so a menu
    entry and a shortcut cannot come to different answers.

- [x] 16.2 Measure what each entry actually does
  - `masking.rs` called Inverter, Suavizar and the bounded complement and
    asserted nothing whatever about them. Now, per entry:
    - **Inverter** takes the middle of a patch from 0.992 to 0.008 and the clay
      beside it from 0.000 to 1.000 — and reaches only where the mask has been
      allocated, leaving the far side of the model free. Written down rather
      than discovered: it is what makes the operation finite and why the
      bounded complement is a separate entry.
    - **Expandir** and **Contrair** move the cell count in opposite directions
      and further at four than at one; three of each in sequence returns the
      patch to within a tenth of where it started, which grey dilation followed
      by erosion is not obliged to do exactly.
    - **Suavizar máscara** brings the middle down (0.992 → 0.980 at one pass,
      0.898 at eight) and spreads the boundary, and softens rather than erases.
    - **Complemento delimitado** frees the middle and freezes the shoulder
      while leaving the far side alone, which is the whole difference from
      Inverter.
    - **Extrudar** puts the patch in a layer of its own and leaves the mask
      intact. On a unit sphere with a 0.2 wall: Para fora reaches 1.16, Para
      dentro leaves the outside at 1.000, Centrado reaches 1.1015 — half the
      thickness above the surface. Para fora is *not* base plus thickness, so
      it is held to an ordering rather than to arithmetic it does not obey.
  - Driven with real clicks at the shell as well: the Máscaras menu is opened,
    Expandir is clicked, and the command that comes out has to carry the
    panel's five. A menu entry that draws and is wired to nothing looks
    identical, and so does a slider.

- [x] 16.3 Make Extrudar work where it can, and say so where it cannot
  - Reported as not working, and it was not: every test above ran on an SDF
    layer, and `clay_document_mask_extrude` samples a *layer's field*. On the
    mesh layer a sculptor is most likely to be on it refused outright — "this
    layer has no field to extrude from" — and so did a grid.
  - Worse, the refusal was invisible. `MaskViewModel::notice` was an
    `Observable` nothing read, so the entry was a click that did nothing at all
    and said nothing at all. It reaches the options bar's status line now,
    which is the one place the application already uses to say why something
    did not happen.
  - A grid has its own verb, `clay_voxel_mask_extrude`, wrapped in `claycore`
    since the beginning and never bound. It works from the cells the grid
    already knows are on its surface rather than through a sampled field, which
    is what avoids a conversion. Measured: a 0.2 wall on a grid whose surface
    sits at 0.100 reaches 0.246. Both paths produce an SDF row, so Extrudar
    means one thing whatever it was run on.
  - A mesh has neither verb. `can_extrude` lives in the domain and the menu
    asks it before offering the entry, so what a sculptor meets is a grey item
    whose reason names the crossing that would let it work. Held by a shell
    test that clicks the entry on both and checks that one emits a command and
    the other does not.

## 17. The deformation cage

- [x] 17.1 Bind both lattice routes
  - `clay_mesh_sculptor_lattice` was wrapped in `claycore` and reached by
    nothing above it. `clay_layer_lattice_gizmo`, the field's own route, was
    not wrapped at all — `GizmoCage` and the two entry points are new.
  - The ceilings differ and the difference is the mechanism: a mesh is deformed
    forward, each vertex evaluated once, up to 32 points per axis; a field by
    an inverse point map resolved into one deformer per item and evaluated at
    every sample, which the engine caps at 4. A grid has neither and is refused
    with the crossing named, as Extrudar now is.
  - The domain owns both numbers (`division_limit`) so the panel's one slider
    clamps to the active layer's ceiling rather than the View guessing.

- [x] 17.2 Draw the cage, and let the pointer reach it
  - Lines along the cage's edges and a small box at every control point, in the
    overlay pass the rig uses — both are scaffolding, and scaffolding occluded
    by the thing it annotates is not scaffolding. The selected handle is drawn
    larger as well as brighter, so which point is in hand is legible without
    reading the colour, which a sculptor watching the form is not doing.
  - The handle's size comes from the cage's own extent, so a cage around a
    thumbnail and one around a bust both get a handle a person can hit. The
    grab radius is wider than what is drawn: a handle you can see and cannot
    hit is worse than one drawn a little small.
  - A press on a control point takes the primary button *before* the surface is
    asked about. A control point sits outside the form, so a press on a corner
    handle would otherwise find the clay behind it and start a stroke on the
    layer the cage exists to bend.

- [x] 17.3 Two defects found on the way
  - `bounds` answers from a layer's *SDF* extent, which a mesh layer does not
    have — the first cage over a mesh was refused as an empty layer. Measured
    from the mesh's own vertices now. The same shape as the voxel Frame All
    defect already recorded in group 10.
  - The inspector grew past the fold. Adding a cage section above the mask's
    moved the Passos slider out of the visible panel, which the mask test
    caught only because it reached the control by pixel coordinate — and it
    then hit the cage's slider instead and passed for the wrong reason. Sliders
    carry a stable id now and tests find them by name; the cage is raised from
    the Dinâmica menu, which was empty, and its panel section appears only
    while a cage is up.

**Still open**: the transform gizmo — translate, rotate and scale on the
selection — and with it selecting more than one control point at a time. A
gizmo is what makes a multi-point selection worth having.

- [x] 17.4 The manipulator, on a selection
  - What makes selecting more than one control point worth having. A click
    replaces the selection, Shift-click adds or removes one, and the widget
    sits on the selection's *middle* — not on the last point picked, so adding
    a point moves the widget to where the selection is.
  - One widget with three modes rather than three widgets, which is what ZBrush
    and Maya both settled on. Shapes rather than colours alone carry the
    meaning — an arrow slides, a ring turns, a box scales — because a person
    reaching for a handle is not reading a legend, and the three axis colours
    are the one part of this a colour-blind sculptor cannot use.
  - The arithmetic is a free function on `GizmoDrag` with no viewport in sight,
    which is the part worth checking directly: a quarter turn is a quarter
    turn, an axis drag stays on its axis, a scale about a pivot is about that
    pivot, and every mode leaves a point alone when the drag has not moved.
  - Three decisions the tests hold rather than the code merely implying:
    - A **scale never passes through zero**, either way. A drag that overshot
      the pivot would turn the form inside out with no way back but undo, and a
      drag that started very near the pivot has a tiny denominator — without a
      ceiling an ordinary pull produces a factor in the thousands.
    - A drag is **resolved from its anchor every frame**. Transforming what the
      last frame produced compounds a rotation into a spiral and a scale into a
      runaway. The first version of the test asserted this wrongly — two
      *separate* gestures should compound, and it claimed they should not — so
      it now compares a drag that wandered on its way against one that went
      straight to the same place.
    - Grabbing a ring **on its own axis does not spin it**: "which way round"
      has no answer there, and a manipulator that spun when grabbed at its
      centre would be unusable.
  - An axis drag runs on the plane containing that axis and most nearly facing
    the eye. A plane facing the camera outright would make an axis pointing at
    the viewer unmovable — the pointer could travel a long way and its
    projection onto the axis would barely change.
  - Press order in the viewport is manipulator, then control points, then the
    surface, and each step is there for a reason: the manipulator is drawn over
    the cage and sits on the selection, and the cage sits outside the form.

**Still open**: the manipulator acts on a lattice selection and nothing else.
Transforming a whole layer with it — the other thing both references use a
gizmo for — has no route here yet.

- [x] 17.5 Show the bend while the cage is dragged
  - A cage that showed nothing until Deformar made the sculptor aim blind: set
    every corner, press, look, start again. On a mesh the form follows the
    pointer now, through the same machinery a mesh stroke's preview uses —
    `MeshDeltas` held rather than banked, taken back and laid down again from
    the mesh as it was.
  - Taking the previous preview back first is not optional. The lattice is
    *absolute* — offsets from rest, evaluated against the original vertices —
    so laying it over a surface an earlier frame already bent doubles the
    deformation on every pointer move. Measured across twenty frames of one
    drag against a single frame of the same drag, which is what the test
    compares.
  - **11.2 ms** a frame on 62,576 vertices, and the revision moves every frame
    so the viewport actually re-uploads — a mesh layer is not in the brick
    cache, so nothing else about the edit would say the surface had moved.
  - **No preview on a field**, and that is a cost rather than an oversight: the
    field route writes a lattice deformer into the document as an undoable edit
    and refills the layer's whole brick region, **68.8 ms** for one apply on
    the starting form. There the cage moves live and the surface follows when
    it is applied.
  - Applying takes the preview back and lays the cage down once more, banked,
    rather than keeping what is on screen: a preview holds the deltas of one
    pass, and turning that into the edit would leave the undo stack describing
    a gesture rather than a deformation.

- [x] 17.6 Preview a field cage too
  - Reported after 17.5 shipped: the mesh preview was visible and the field one
    was not. It was documented as a cost, and it did not have to be one.
  - `clay_mesh_lattice_displacement` — "exposed so a host can preview the warp
    without applying it" — was not wrapped. It is now, and the preview moves
    the vertices the viewport already holds rather than touching the document,
    so no lattice arithmetic is written twice and nothing is recorded.
  - It is the **forward** map where the field's own deformer is the inverse
    one, so the size of the difference is the whole question. Measured against
    the engine's own result on a cage spanning ±1.1: **0.6% of the drag** at
    0.05, 0.10 and 0.25, and 16% at 0.50 — a drag most of the way across the
    box. A test holds the preview to under 5% at a quarter-box drag. What lands
    on Deformar is the engine's, computed the engine's way.
  - Two things measured on the way that are worth having written down:
    - The engine's header quotes the forward/inverse difference as "under 1.5%
      of the drag". At a drag of 45% of the cage's half-width it is 16%. The
      header's number is a measurement of their case, not a bound.
    - A probe that showed the same applied result for a 2×2×2 corner drag and a
      4×4×4 all-layer drag looked like the field cage doing no FFD at all.
      Dragging a *single* corner settled it: +z reaches 1.2 while −z and ±x
      barely move. The earlier probe dragged every +z point, which on a
      symmetric sphere gives nearly the same answer at any resolution.
  - The rest positions are kept so the surface can be put back, and dropped on
    a re-mesh: the vertices they described are gone, and the next preview
    stores them again from what is there now.

- [x] 17.7 Make a cage behave like a mode
  - Three things reported from using it, and none of them was treating a cage
    as the mode it is.
  - **The brushes kept working.** A press that missed a control point fell
    through to the brush, so a slip while aiming sculpted the very form the
    cage was there to bend — and the blobs it left made the next point harder
    to hit. The rule moved out of the event loop into `input::press_sculpts`,
    with its reasons and three tests: a rule with three clauses and no test is
    how a mode stops being a mode. It orbits rather than doing nothing, so a
    cage can still be turned to look at from behind.
  - **The form is drawn through** while a cage is up. Half the control points
    are behind it and a solid surface hides exactly the handles that need
    reaching. A `fs_ghost` entry and a pipeline with no back-face culling — and
    so no depth write — which is what lets the far half of the cage read
    through. Held to being *seen through* rather than turned off: a test
    requires the ghosted form to still cover four fifths of what the solid one
    did.
  - **Handles inflated as the cage grew.** The size came from the cage's
    current extent, so hauling one corner out grew every other handle and the
    targets a sculptor was aiming at swelled under the pointer. `rest_span`
    carries the box the cage was built with, and the size comes from that.

## 18. Symmetry, on the two representations that had none

- [x] 18.1 Give a mesh stroke its symmetry axes
  - It did nothing at all. `apply_stroke` takes the enabled axes and the mesh
    arm of the dispatch dropped them — `stroke_mesh` was not even *given*
    them — so every X, Y and Z button was inert on a mesh while working on a
    field.
  - No engine-side mesh symmetry to reach for: `clay_set_layer_mirror` reflects
    a layer's *items*, and a mesh layer has vertices. Both references do the
    same thing in that position, so the stroke is mirrored and applied again.
  - Measured in Blender 5.2 on a 64×32 sphere, one Draw dab: 82 vertices on +x
    with symmetry off; 82 on each side with X on; 161 in each of four quadrants
    with X and Y on. So one dab per reflection at full strength, and the full
    subset lattice — two axes give four dabs, three give eight.
  - Getting Blender to say that took two corrections. `brush_stroke` needs a
    start *and* a step, not one sample. And in 5.2 sculpt symmetry lives on the
    **mesh** (`use_mirror_x`), not only on `tool_settings.sculpt` — set on the
    tool settings alone it reads back as enabled and does nothing, which is
    what made three runs return identical numbers.
  - A reflection turns a direction over as well as a position. The mirrored
    Grab's `direction`, the stroke path and the stamp centre all reflect; the
    deposit normal and the alpha plane are left at all-zeroes, which means "the
    region's own normal" and so mirrors with the surface.
  - Every reflection into the same `MeshDeltas`, so a symmetric stroke is one
    undo and the preview's revert takes every copy back together.

- [x] 18.2 The same for a grid
  - The voxel arm dropped the axes too, for the same reason and with the same
    effect. A grid has no layer mirror either; its cell lattice already puts a
    plane at coordinate zero. The smudge direction reflects with the stroke.

- [x] 18.3 Measure the form, not the vertices
  - The obvious test — count what moved on each side — is wrong here, and
    finding out why was worth the detour. Our mesh comes from marching cubes,
    whose vertex density is not the same on both sides of a plane: a lone dab
    moves 497 vertices at one place and 272 at its mirror. A lone dab *at* the
    mirror moves 272 as well, which is what says the mirroring is exact and the
    difference belongs to the tessellation. Blender's UV sphere is symmetric by
    construction, hence its 82/82.
  - So the tests measure the surface at mirrored places rather than counting
    vertices, which is also what a sculptor means by symmetry. The voxel test
    measures the deposit's extent for the same reason: the greedy mesher merges
    quads differently either side of the seam, giving 164 vertices against 152
    for a deposit that is exactly symmetric.

## 19. A language, and a way to choose it

- [x] 19.1 Put the language in a menu
  - Three complete translations — pt-BR, en-US, es-419 — shipped from the
    beginning with no way to choose between them. The locale came from
    `Locale::default()` at startup and was never asked about again, so
    `Locale::from_tag`, written for exactly this, was called by nothing.
  - **Vista → Idioma**, each language named in itself. That is the one rule a
    language menu has: a reader who cannot read the current interface still has
    to be able to find their own.
  - `Locale` moved from `clayspace-view` into `clayspace-model`, beside the
    display unit it is a sibling of. A language is a *preference*, and a View
    may not own a type a Command has to carry — the layering check would have
    said so.
  - `Strings` carries its own locale now, so the menu's tick and the words on
    screen cannot disagree about what the interface is in.

- [x] 19.2 Open in English, and honour the system before that
  - The default was Brazilian Portuguese, which is the design's own language
    and not what a first-time reader can necessarily make sense of. English
    now, with the reason written where the default is.
  - A system tag still wins on a first run: `LC_ALL`, `LC_MESSAGES` or `LANG`,
    read from the environment rather than through a crate — three variables are
    not worth a dependency to audit and license. Matched by language rather
    than region, so `pt_PT`, `en_GB` and `es_ES` each find their translation.
  - The choice is written to the session directory as a tag and read back only
    if it is one of the three: `from_tag` answers with the default for anything
    else, and taking that would turn a corrupted file into a preference nobody
    set. A test round-trips every locale through its tag.

**Still open**: the *vocabulary* the domain names — 82 label arms across 15
enums, from `ToolKind` and `Combine` to `MaskOp` and `GizmoMode` — are
Portuguese literals returned from `clayspace-model` rather than looked up in
the string tables. Switching language translates the chrome and leaves the
brush shelf and the option bar in Portuguese. Routing them through the tables
is a change of its own.

- [x] 19.3 Translate the brush names
  - Checked as reported and true: the shelf drew `ToolKind::label()` — the
    domain's own Portuguese — on SDF, voxel and mesh alike, whatever the
    interface language was. So did the status bar's last action. All twenty.
  - `Strings::tool` looks them up per locale from a fixed-length array in
    `ToolKind::ALL`'s order, so a tool added without a name for it is a compile
    error. Here rather than on `ToolKind` because a name is a *word* and the
    domain has no language; `ToolKind::label` keeps its Portuguese for history
    entries, engine refusals and the diagnostics report.
  - `LastAction` carries the `ToolKind` beside the label rather than instead of
    it: the View names the tool from its own table, and the label stays for the
    actions no tool made and for the log.
  - **A false friend, written down because it reads as correct to anyone
    checking one language at a time**: Portuguese `Borrar` is *smear* and
    Spanish `Borrar` is *erase*. Carried straight across, the Spanish shelf
    would name the smudge brush "erase" and leave the erase brush with the
    smudge's name — two brushes, both wrong. They swap: `Apagar` → `Borrar`,
    `Borrar` → `Difuminar`. A test holds it.
  - Tests: every brush has a distinct non-empty name in every language; the
    names are translated rather than copied (at least fifteen of twenty differ
    between Portuguese and English, eight between Portuguese and Spanish); the
    Portuguese table agrees with the domain, which is what makes
    `ToolKind::label` safe to keep using off the interface; and the rendered
    shelf differs between languages, measured on the shelf band alone so it is
    about the brushes rather than the panels.

**Still open**: 62 further label arms across 14 enums — `Combine`,
`BlendProfile`, `ViewPresetKind`, `MaskOp`, `GizmoMode`, `ExtrudeSide`,
`Falloff` — leaving the option bar and the viewport bar Portuguese.
`Strings::tool` is the shape the rest should follow.

## 20. Every SDF brush: it works, it takes a sign, it mirrors

- [x] 20.1 Symmetry, which was broken both ways
  - Reported as "Smooth has no symmetry on SDF". True, and it was **six**
    brushes rather than one: the surface drag, both relaxes, both planes and
    the snakehook all bypassed `stroke_sdf` and so were never handed the axes.
  - Symmetry on a field is the layer mirror, which reflects a layer's *items*.
    Five of the six **rewrite the field** instead, and the mirror cannot reach
    those even when it is on — measured, a relax with X mirrored took the
    surface under the stroke from 1.1467 to 1.1409 and left its reflection at
    1.1467 exactly. Their strokes are reflected instead, reusing the `mirrors`
    helper the mesh and voxel paths already use.
  - The sixth, the snakehook, adds items, so the mirror does reach it. Its
    fault pointed the other way and would have gone on being invisible: never
    *setting* the mirror, it inherited whatever was last asked for, and the
    starting form turns X on. A snakehook with symmetry switched **off** came
    out on both sides at 1.4625. `point_the_mirror` is called for every SDF
    stroke now, and a test asserts the far side is untouched with symmetry off.

- [x] 20.2 The sign, where a brush has one
  - Depositing already inverted. **Planing did not, and could**: cut-only is
    what a planing tool wants — it must not fill the dents it is meant to
    reveal — and `FlattenMode::FillOnly` is the other half, in the engine all
    along and never asked for. Measured on a sphere with a bump and a dent
    beside it: upright takes the bump from 1.1150 to 1.1145 and leaves the
    hollow at 0.8923; held, it fills the hollow to 0.9004 and leaves the bump
    exactly where it was.
  - The rest have no opposite, and that is now stated rather than left as an
    absence: an inverted smooth is not a thing either reference offers, and a
    drag's direction already *is* its sign. A test asserts the key changes
    nothing for those, so a brush that quietly gained an opposite would be
    caught.

- [x] 20.3 Look at each of them
  - `visual_sdf_symmetry.rs` draws all nine surface brushes mirrored, head-on
    so the mirror plane is the middle column, and compares each picture with
    its own reflection.
  - Two corrections to the instrument, both worth keeping:
    - Comparing **colour** scores a perfectly mirrored dab at 0.58, because a
      MatCap shades by the view-space normal and is not itself left-right
      symmetric. The **silhouette** is what two halves of a symmetric form
      share; by that measure all nine sit at 0.004.
    - A bump facing the camera does not change the outline at all, and the bake
      verbs move a bare sphere by about 0.006. The stroke runs along the limb
      and the subject is roughened first, which is also the only case in which
      a sculptor reaches for a relax.
  - The control — that a brush asked for no symmetry stays on one side — is
    kept to the two whose work a silhouette can resolve. One-sidedness in a
    0.006 bake is measured rather than looked at; `sdf_brushes.rs` holds all
    nine with a raycast.

- [x] 20.4 A regression the existing tests caught
  - `visual_bake_tools` went from 5.4 to 6.0 roughness. Verified against main
    before assuming anything, and it was mine.
  - Its fixture roughened by calling the engine directly with symmetry **off**
    and then sculpted through a ViewModel whose default is X **on**. The bake
    tools used to ignore that; now they honour it, so the bake turned the layer
    mirror on under itself and added a reflected copy of the roughening on top
    of the original. No sculptor's session changes its mirror halfway.
  - The fixture is unmirrored throughout now, on both sides of the seam, which
    is what the recorded roughness ceiling was calibrated on. Symmetry is
    measured in `sdf_brushes.rs`, which is where it belongs.

## 21. Every voxel brush: it works, it takes a sign, it mirrors

- [x] 21.1 Symmetry already worked; the sign did not
  - The same three questions asked of the SDF shelf, and a different answer.
    Voxel symmetry landed with the mesh's — a grid has no layer mirror either,
    so its strokes are reflected — and all eight shaping brushes mirror. What
    was missing was the opposite.
  - Three engine verbs come in documented pairs and only one half of each was
    ever asked for: `sculpt_inflate` (*"amount > 0 dilates, < 0 erodes"*, and
    the binding passed a hard `1`), `sculpt_magnify` (*"pinch's inverse,
    sharing its walk so the two cannot drift apart"*, wrapped in `claycore` and
    reached by nothing), and `erase_brush` against `set_brush` for Apagar,
    whose upright verb is the removal so its opposite runs the other way round.
  - A fourth looked like a pair and is not. Turning the scrape's normal over
    moved 2580 indices to 2568 — both directions removing, because the normal
    there is a fixed up-vector rather than the surface's own. Left unbound: a
    guess dressed as a feature is worse than an honest absence.
  - Pintar is inert and honest about it — 0 of 1848 vertex colours changed and
    `changed: false` reported. The palette holds one entry because nothing
    chooses a brush colour, so it paints cells the colour they already are.

- [x] 21.2 Two fixtures that were lying, and one instrument that could not
  - The first fixture packed material only where the stroke lands. Half the
    voxel verbs *reshape* rather than deposit, so the mirrored copy met an
    empty grid and every reshaping brush read as "symmetry does nothing" —
    the fixture's fault. A slab across the whole of x fixed it.
  - The first symmetry metric asked whether the far side *grew*. Suavizar's far
    side went from 814 vertices to 518: it was smoothed, not skipped. Asking
    whether it *changed* is the question.
  - And pixels turned out to be the wrong instrument for a grid altogether. A
    perfectly mirrored deposit scores 0.33 against its own reflection — cells
    are cubes, the greedy mesher merges quads differently either side of the
    seam, and a MatCap-lit blocky ribbon in perspective is not pixel-symmetric.
    The relative form does not hold either: mirroring lowers the score for five
    of six and *raises* it for Pinçar, 0.4189 against 0.3834. The visual test
    captures both frames per brush and asserts only that each reached the
    screen; symmetry is measured on the cells in `voxel_brushes.rs`.
  - Three dead ends before that, each ruled out by measurement rather than
    argument: the dither (every verb dithers below full strength against a hash
    of the *cell coordinate*, which is not symmetric), the wobble in the slab
    (not an even function of x), and the camera (framing a slab's own bounds
    puts it off the mirror plane).

## 22. Boxes or a surface

- [x] 22.1 Draw a grid as the sculpt rather than as its cells
  - Asked whether a voxel layer should be drawn as small cubes, with a comment
    proposing marching cubes or dual contouring. Directionally right and wrong
    on three specifics, and the check was worth making before writing anything:
    - **The algorithm is not missing.** `clay_voxel_mesh_smooth` is surface
      nets — the dual method — and has been there all along. Nothing needed
      writing here or in ClayCore.
    - **It is not a replacement.** The engine says the boxy picture is "correct
      for hard-surface voxel work and for export, and the wrong picture of an
      organic sculpt", and keeps the choice an argument so "one host can show
      both pictures of one sculpt without mutating it". A display setting, then
      — which is also what 3DCoat offers.
    - **It is not dual contouring.** A vertex sits at the centroid of its
      cell's edge crossings, so corners *round*. Dual contouring fits the
      vertex by least squares to hermite data and keeps them sharp. The
      comment's "preserves sharp features" is not what this gives, and getting
      it would be an engine change.
  - Measured on one grid: greedy 6828 verts / 3414 tris / 1.5 ms; smooth 2221 /
    4980 / 16.8 ms; smooth with one blur pass 992 / 1992 / 19.0 ms.

- [x] 22.2 The two facts that shaped the wiring
  - **The smooth mesh carries no normals.** Colour blends across it — a vertex
    sits between up to eight voxels and averages the occupied ones, there being
    no facet to hold one palette entry — but a normal is the host's to work
    out. Without them the surface renders as a flat silhouette, which is
    exactly what an earlier attempt at this looked like and why it was dropped.
    Computed area-weighted in the bridge, with a test that would have caught
    it: the rendered surface must show more than twenty distinct tones.
  - **It cannot be meshed a chunk at a time.** `clay_voxel_mesh_chunks` is the
    greedy mesher alone, and the engine explains why: greedy quads are
    axis-aligned and exact, so clamping their merge to a chunk boundary emits
    more, smaller quads over the identical surface and never a crack, while
    surface nets place a vertex from a cell's *neighbourhood* and would tear.
    So the smooth picture is a **settle** — rebuilt when a gesture ends, beside
    the brick surface's own settle — because the incremental boxy path is
    3.3 ms a dab against 309 ms for a whole-grid re-mesh.
  - The smooth mesh counts toward `mesh_revision`, or a settle that rebuilt it
    without touching a cell would never reach the viewport.

- [x] 22.3 Say what the filtering costs
  - `blur` is the engine's, in passes of a 3×3×3 box over occupancy. At 0
    nothing is filtered and nothing is lost, but the surface still terraces; at
    1 it reads as clay and an isolated voxel sits under the isolevel and is
    gone. Default 0, in the engine's own words: "a default that silently
    deletes a sculptor's detail is the wrong default however good it looks."
  - `SmoothBlur::can_lose_detail` is asked by the interface, which says so in
    the accent colour where it is true — rather than leaving a sculptor to find
    out from a missing finger.

- [x] 22.4 Make the form the default, and make it keep up
  - Asked for the smooth surface always rather than as an option. A sculptor is
    shaping a form, not a lattice, and the cells a grid is stored in are a fact
    about the storage — showing them by default made a voxel layer the odd one
    out for a reason that belongs to how it is kept. The boxes stay on the
    toggle, because seeing the cells is sometimes exactly what is wanted and
    because they are what exports.
  - The settle was the right home for the rebuild while the boxes were the
    default and the wrong one the moment the smooth surface is what a sculptor
    watches: a form that waited for the pointer to come up would lag a whole
    gesture behind the brush. It is rebuilt where the geometry is assembled
    now, **guarded on the grid's own change count** — a frame in which nothing
    moved costs one comparison. Measured, a whole-grid smooth mesh is 17.3 ms
    at a 0.05 voxel size, 18.0 at 0.03 and 20.6 at 0.02: flat enough in the
    size of the grid to sit on the frame path.
  - The rebuild lives inside `visible_mesh_geometry`, beside the chunk refresh
    it already does, rather than being left to the caller. That method's job is
    to hand back what the viewport draws, and a consumer that did not know to
    ask would silently have got the boxes.
  - Changing the *blur* drops the stored mesh rather than comparing: the
    filtering changed, so what is held is stale even though no cell moved and
    its change count still matches.
  - Two tests for the pair of it: a dab reaches the smooth surface with no
    settle and no explicit rebuild, and an untouched grid is not meshed again —
    its surface and its revision both sit still, or the viewport would
    re-upload the same form every frame.
  - Checked visually as asked. Every voxel brush changes the smooth picture
    except the two that should not: Máscara paints a freeze and Pintar has no
    colour to paint with. Padrão and Camada move ~1840 pixels, Inflar 1094,
    Apagar 448, Pinçar 432, Nudge 427, Raspar 264, Suavizar 243.

## 23. The tendril a snakehook pulls

- [x] 23.1 One gesture, one curve
  - It came out a string of beads. Puxar authors a *curve* — a swept-sphere
    chain tapering to its tip — and a drag arrives in segments, so each segment
    authored its own item and restarted the taper from full width. A curving
    pull left a chain of spheres.
  - The gesture holds the curve it is pulling and replaces its points through
    `clay_layer_set_stroke_points`, which was in the engine and unwrapped.
    Measured on one curving pull, the thickness along it wobbled by **0.210**
    before and **0.122** after — and a single tapering curve wobbles 0.137 from
    the taper alone, so the fix is under that.
  - The ViewModel replays a field snakehook from its anchor now, as a mesh drag
    already did. That is only safe because the model *grows* the one curve
    rather than adding another; replaying onto the old behaviour would have
    stacked a tendril per segment.
  - The curve is dropped when the gesture ends, so the next pull is its own.

- [x] 23.2 A curve rather than a chain of corners
  - A stroke's points are hard corners by default — a straight segment to the
    next — which the engine says is "exactly what a chain authored before types
    existed means". For a path a pointer traced it is wrong: every sample is a
    kink and the swept sphere bulges at each one.
  - `clay_item_set_curve_points` takes a type per point and was unwrapped.
    Catmull-Rom passes *through* the points, so the tendril is the path the
    pointer took, and the engine tessellates typed points into the same segment
    chain at compile time — so it costs nothing at evaluation.
  - A straight drag hides this completely, which is why the first probe found
    nothing. A curving one is where it shows.

**Asked alongside**: whether Nomad's Tube would be possible today. Every piece
is in the engine — `CLAY_PRIM_SWEPT` sweeps a profile along a guide curve,
`clay_item_add_loft_profile` supplies seven profile kinds including arbitrary
polygons, `clay_item_set_curve_points` types each point hard, Catmull-Rom,
B-spline or Bezier with handles, and `clay_layer_set_stroke_points` edits a
placed curve undoably. What is missing is entirely on this side: a tool that
places a curve, draws its control points and handles, and lets them be dragged.
Recorded under *Not built yet*.
