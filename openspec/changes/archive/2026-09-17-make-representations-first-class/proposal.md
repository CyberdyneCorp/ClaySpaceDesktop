## Why

ClayCore carries three representations side by side and its own guidance is that
the intended workflow crosses them: **block out and hard-surface on SDF,
free-form sculpt on voxels, refine on a mesh when the topology is one you want
to keep.** This application makes a layer's representation something you choose
once and then live inside. There is no way across, and one of the three cannot
be sculpted at all.

That was an accurate reading of the engine when it was written. It is not one
now. ClayCore 0.39.0 sculpts a mesh layer's own vertices with sixteen
fixed-topology brushes, converts in every direction including mesh straight to
voxels, and keeps an addressable stack of sculpt layers on a grid. Measured
against the engine's vocabulary, the application reaches a minority of it: 15
tools against 16 combine ops, 21 deformers, 10 voxel verbs, 16 mesh brushes and
four conversions.

The gap is not a list of missing buttons. It is that the shell has no concept of
what the active layer *is*, so there is nowhere for a representation's own verbs
to live and nothing to disable them against except a hard-coded refusal.

## What Changes

Delivered in three phases, each shippable on its own.

**Phase 1 — the shell learns what a layer is.**
- The active layer's representation becomes a first-class piece of shell state,
  surfaced in the viewport bar and the layer stack.
- The tool shelf presents the verbs that exist for the active representation,
  rather than one list with some entries greyed out.
- Tool availability stops being a hard-coded refusal and becomes a per-verb,
  per-representation table the engine's own capabilities feed.
- **BREAKING** (behaviour): a tool that does not exist for the active
  representation is absent rather than shown disabled, so the shelf's contents
  change when the active layer changes.

**Phase 2 — conversions become commands.**
- SDF → voxel (rasterize into a region), voxel → SDF (return as an operand,
  or a whole coloured sculpt as a new layer), mesh → voxel (direct, one
  sampling), mesh → SDF (the existing import path, made explicit).
- Every conversion states what it costs *before* it runs — surface movement of
  about half a cell, features thinner than a cell, sharp edges to a staircase,
  and the procedural history not coming back.
- A conversion produces a new layer rather than replacing one, so the original
  stays where it was.

**Phase 3 — the missing vocabulary.**
- **Mesh sculpting**: the sixteen fixed-topology brushes, taper and twist, the
  lattice cage, and paint/smear on vertex colour. Topology never changes.
- **Voxel**: the verbs not yet reached (carve-with-alpha, flood select, box and
  line fill, paint/erase with falloff), pre-bake repair (report, close holes,
  fill voids), and the resolution level stack including regional refinement.
- **Voxel sculpt layers**: bracket a run of strokes and keep their strength
  adjustable afterwards — addressable rather than a stack you pop.
- **SDF**: alphas as a stamp source, the deformers as authoring operations, and
  the five blend profiles exposed where a combine op is chosen.
- Masks gate *operations* and not only brushes, which is what the engine's gate
  record already supports.

## Capabilities

### New Capabilities
- `representation-modes`: what the active representation is, how it is shown,
  and how the shell's tools and panels follow it.
- `representation-conversion`: converting a layer between SDF, voxel and mesh,
  and stating what each conversion costs.
- `mesh-sculpting`: sculpting a mesh layer's own vertices under the guarantee
  that topology never changes.
- `voxel-sculpt-layers`: an addressable stack of recorded strokes on a grid
  whose strength stays editable.

### Modified Capabilities
- `scene-and-layers`: **Mesh layers are carried but not sculpted** is
  overturned. A mesh layer is sculptable, pickable and colour-editable; what it
  still cannot do is compose — it is not an operand until it is converted.
- `sculpting-tools`: the tool set is scoped by representation rather than being
  one list with a disabled reason, and grows to cover the engine's vocabulary.
  **Every tool maps to a documented engine verb** stands and is what the growth
  is measured against.

## Impact

- `clayspace-model`: `ToolKind` grows and stops being one flat enum across
  representations; `Representation` gains the capability table.
- `clayspace-vm`: new commands for conversions, mesh brushes, voxel repair and
  sculpt layers; `SculptModel` gains the operations that are not strokes.
- `clayspace-engine`: bindings for `clay_mesh_sculptor_*`,
  `clay_voxel_sculpt_layer_*`, `clay_voxel_rasterize*`,
  `clay_item_volume_from_voxels`, `clay_voxel_to_layer`, the repair verbs and
  the alpha entry points.
- `clayspace-view`: the shelf becomes representation-scoped; new panels for the
  sculpt-layer stack, conversion, and repair.
- `claycore`: safe wrappers for the above, which is where the `unsafe` stays.
- Documentation: `docs/features.md`'s *Deliberately absent — mesh-surface
  brushes* entry and its *Not built yet* section; `README.md`'s input table.
- Engine floor rises to ClayCore 0.39.0, already pinned.
