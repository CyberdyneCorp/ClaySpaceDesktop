## Why

The engine and the model are already multi-object — a document holds N layers,
each independently an SDF tree, voxel grid or mesh, each with its own
transform, visibility, protection and mirror — but the application still
treats the stack as one sculpt plus decorations. There is no way to put a
second form in the scene and work it on its own: a placed primitive becomes an
item *inside* the active layer, clicking a shape does not make its layer the
sculpt target, the whole-layer manipulator is implemented and unreachable from
any control, a new layer is always SDF, and one mask, one symmetry setting and
one armature are shared by the whole document. ZBrush calls the missing
workflow *subtools*: a scene is a list of separate objects, each transformed,
sculpted and deformed on its own, and combined with the others when the
sculptor says so. Everything it needs below the UI already exists here or in
ClayCore 0.52.2.

## What Changes

- **Inserting a form creates a subtool.** The fourteen bounded primitives, an
  imported mesh, and a copy of a subtool already in the scene each arrive as
  their own layer — selected, transformable and immediately sculptable —
  rather than as an item inside whatever layer was active. Placing *into* the
  active layer stays available for the parts that make up one form.
- **Booleans between subtools produce a subtool.** Union, subtract and
  intersect over two subtools bake to a new subtool carrying the result, which
  is then an ordinary subtool: sculptable, transformable, an operand again.
  The engine composes layers by hard union only (ClayCore
  [#321](https://github.com/CyberdyneCorp/ClayCore/issues/321)), so this is a
  *resolved* boolean rather than a live one — its cost is stated before it
  runs, as every other crossing in this application already is, and the
  operands are kept by default.
- **Clicking geometry activates its subtool.** The attributed raycast already
  answers which layer was hit (`SceneModel::select_at`, built and unwired);
  selecting a subtool in the viewport or the stack makes it the layer dabs
  land on. Ghosted layers stay transparent to the pick, as the engine already
  enforces.
- **The whole-subtool manipulator becomes reachable.** `GizmoTarget::Layer` is
  implemented and tested and no view code pushes it — move, turn and uniform
  scale on a whole subtool, closing the already-specced "A whole layer is
  turned" scenario rather than changing it.
- **Sculpting and deforming follow the active subtool.** Brushes already do;
  the deformation cage already sizes itself to the active layer. A cage left
  standing when the sculptor switches subtools is resolved explicitly instead
  of silently following them across.
- **Adding a subtool asks which representation**, and **solo** shows one alone
  and brings the rest back.
- **Per-subtool sculpting state.** Mask, symmetry axes and armature move from
  `ClayDocument`'s per-document singletons into the layer they belong to, so
  switching subtools restores that subtool's mask, mirror and rig. The engine
  is already per-layer for all three.
- **The active subtool reads as active.** The renderer keeps per-layer submesh
  ranges so the active subtool can be tinted and an inactive one dimmed,
  matching the stack.

Out of scope, tracked upstream or deferred: a *live* boolean between subtools
that re-evaluates as an operand moves (ClayCore #321), duplicate-subtool by
instancing rather than by copy
([#364](https://github.com/CyberdyneCorp/ClayCore/issues/364), filed from this
proposal), retiring the voxel unique-name rule
([#365](https://github.com/CyberdyneCorp/ClayCore/issues/365), filed from this
proposal), undo that does not dirty a whole layer
([#210](https://github.com/CyberdyneCorp/ClayCore/issues/210)), and subtool
folders. No ClayCore change is required for anything in scope.

## Capabilities

### New Capabilities

- `subtool-booleans`: resolving a boolean between two subtools into a new
  subtool — what is offered, what it costs, what happens to the operands, and
  what the result is afterwards.

### Modified Capabilities

- `scene-and-layers`: selection gains a consequence — the selected subtool
  becomes the active sculpt target; the stack's add control chooses a
  representation; solo/isolate joins visibility.
- `scene-objects`: inserting a form can create its own subtool as well as
  place an object into the active layer, and the sculptor chooses which.
- `sculpting-tools`: symmetry, masks and armatures become per-layer state that
  follows the active subtool; a standing deformation cage is resolved when the
  active subtool changes.
- `viewport-rendering`: the active subtool is visually distinct from inactive
  ones; carried geometry keeps per-layer ranges to draw that.

## Impact

- `crates/clayspace-model` — `SceneModel`/`ObjectModel` gain insert-as-subtool
  and the subtool boolean, with its cost type alongside the existing crossing
  costs.
- `crates/clayspace-engine/src/document.rs` — the bulk of it: `mask`,
  `symmetry`, `armature` move from `ClayDocument` into `Layer`;
  `set_active_layer` restores the incoming layer's state; the mask switches
  from the standalone `Mask::new` to the engine's per-layer `add_mask`; the
  boolean bakes each operand through `clay_item_volume_from_document` with the
  other layers hidden.
- `crates/clayspace-vm` — wire `SceneViewModel::select_at` and
  `SetGizmoTarget(GizmoTarget::Layer(..))`; new commands for insertion,
  representation choice, solo and the boolean.
- `crates/clayspace-view` — an insert control, the subtool list, the boolean
  panel with its cost, solo, and the active-subtool cue in `renderer.rs`.
- `crates/clayspace-app` — route viewport clicks through subtool activation.
- No changes to `claycore-sys` or the vendored engine; `claycore` may gain thin
  wrappers for ABI calls it already links.
- Tests: a regression test per wiring gap and per boolean outcome; visual
  captures for the cue, the manipulator on a subtool, and each boolean.
  Benchmarks: subtool switching (it pays `arm_mesh_sculptor`) and the boolean
  bake.
