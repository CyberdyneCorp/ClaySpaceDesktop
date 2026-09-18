# Close the brush gaps the pinned engine already answers

## Why

An audit of this application against ClayCore v0.60.0 found that the brush
system is not misusing the engine — mesh sculpting respects the fixed-topology
contract, Grab is a gesture rather than a series of dabs, SDF Move uses the
assembled-surface resolver, symmetry mirrors directions as well as positions —
but that it *does not reach* several verbs the pinned engine has had all along.
The gaps are coverage and plumbing, not semantics:

- **Voxel Paint is inert.** `clay_voxel_paint_brush` is bound and called, but
  nothing anywhere in the application chooses a colour: the adapter paints the
  one clay tone the grid already holds back onto itself, and the renderer is
  told `has_vertex_colors: false` on every frame, so the modulation the MatCap
  and Studio stages both implement is switched off. Mesh Paint has the same
  hole one layer up — `MeshStamp::colour` is left at its `[1.0; 3]` default.
  Two brushes on the shelf that cannot change a pixel.
- **`VoxelField::sculpt_grab` is bound and unreachable.** Mover applies to SDF
  and mesh; a voxel layer has no drag verb, though the engine has had one since
  before this application existed.
- **`VoxelField::sculpt_flatten` is bound and unreachable.** Planar applies to
  SDF and mesh; the engine's own note on `clay_item_volume_flatten` says voxel
  grids "have had `clay_voxel_sculpt_flatten` all along".
- **`Op::Incise` never reaches a tool.** Vinco is mesh-only, though the engine's
  equivalence table maps Crease/DamStandard to `Op::Incise` on a field and the
  op is in this application's own `Combine` vocabulary already.
- **`Op::Relief` with buildup accumulation never reaches Argila.** Clay is
  mesh-only, though the same table maps Clay/ClayBuildup to relief along the
  stroke plus buildup.
- **`clay_item_volume_move_topological` is not bound at all.** Move Topological
  is a distinct brush — a drag weighted by distance *along the material* — and
  the engine measured the difference: on two fingers 0.32 apart joined only
  through a palm, a Euclidean drag at radius 0.5 pulls the far one and this
  does not.
- **Masks do not survive the document.** `clay_document_add_mask` attaches a
  mask to a layer and `clay_document_save` writes it — `claycore_mask_persistence.rs`
  has measured that since task 6.4 — and this application still keeps every
  subtool's mask in a standalone `clay_mask_create` beside the document, so a
  mask is lost the moment a file is closed. The blocker was never the engine:
  it was that `Document::mask` lends a handle borrowing the document while
  every masked verb wants that handle *and* the document together, which Rust
  cannot express. That is a wrapper-API shape, and this change fixes the shape.

## What Changes

- **A brush colour, and two brushes that use it.** One current colour with a
  short recent list, held in the sculpt session rather than beside a tool,
  reaching Voxel Paint through the grid palette and Mesh Paint through
  `MeshStamp::colour`. The renderer stops suppressing vertex colour.
- **Mover reaches voxel layers** through `sculpt_grab`, with a gesture that
  accumulates displacement and emits only what has grown past a cell — the
  engine is explicit that a raw pointer delta under half a voxel per axis
  rounds to no movement.
- **Planar reaches voxel layers** through `sculpt_flatten`, two-sided, which is
  what the voxel verb is and what its tooltip will say. The SDF side stays
  cut-only; a representation-native semantic is not faked into another.
- **Vinco reaches SDF layers** as an `Op::Incise` stroke with a tight profile,
  inverting to a `Relief` ridge, which is what the engine calls its inverse.
- **Argila reaches SDF layers** as an `Op::Relief` stroke with buildup
  accumulation and a dense, clay-profiled preset, distinct from Camada, which
  is the clamped one by definition.
- **A new tool, Mover Topológico**, on SDF layers only, bound to
  `clay_item_volume_move_topological` through a new `claycore` binding. Added
  beside Mover rather than replacing it: the engine documents them as different
  operations, not as modes of one.
- **Masks become document-owned.** `claycore` gains a `MaskSource` that names a
  *layer* instead of lending a handle, and the five masked entry points take it;
  `ClayDocument` stops owning mask geometry and keeps only what the interface
  needs. A mask now survives save and reopen.
- **One resolver, table-tested.** `ToolKind::verbs` stays the single source of
  truth for where a tool applies, and every tool × representation pair the
  table declares is asserted to reach a distinct engine call.

## Out of scope, and why

- **SDF Pinch/Magnify.** `CLAY_DEFORM_MAGNIFY` is per *item* and local, and the
  engine says so in the same paragraph that warns against wiring Move to
  `grab`: on a form blended from several items, magnifying one pulls its share
  and leaves the rest. The C ABI has an assembled-surface resolver for the drag
  (`clay_layer_move_surface`) and none for the radial scale. Reconstructing one
  host-side would put field math in this application, which the architecture
  forbids. Upstream first.
- **SDF stroke alphas.** `clay_layer_apply_stroke` scales its item as a template
  per stamp and the deformer chain does not travel with it —
  `claycore/tests/alpha_deformer.rs` measures it. Upstream.
- **SDF item mask gating.** `clay_item_set_gate` is accepted and inert;
  `claycore/tests/mask_gate.rs` is written to fail the day it works. Unchanged.
- **A true SDF Inflate verb.** The engine has no regional field offset, and the
  guide's own rule applies: do not add an engine verb before a visual
  comparison proves the Relief profile is not enough.
- **SDF Paint.** `Op::Paint` is real and the brick cache can attribute colour,
  but nothing in the surface path carries it to the GPU and the cost of turning
  it on has not been measured. `Combine::offered_for_strokes` keeps excluding
  it, with its reason updated to name the remaining half.
- **A voxel DamStandard recipe.** The engine documents it as a recipe rather
  than a verb, and a preset that only borrows a name is not worth a shelf entry
  until it has been looked at.
- **Ruído / jitter.** `MAX_JITTER` stays at zero. Its cache interaction is a
  separate investigation and mixing it into brush coverage would confuse two
  failures.
