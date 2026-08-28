## Context

See proposal.md — Why. The relevant current state:

- `ClayDocument` (`crates/clayspace-engine/src/document.rs`, 6770 lines) keeps
  `active: usize` as the sculpt target and a separate `selected:
  Option<LayerKey>`; `SceneModel::select_at` resolves a viewport ray to a layer
  through `raycast_attributed` but only sets `selected`, and nothing in the
  view calls it. The only path that changes `active` is the stack row click.
- `GizmoTarget::Layer(LayerKey)` resolves through `place_layer` and carries a
  full `Transform` (position, rotation axis+angle, uniform scale). Covered by
  `crates/clayspace-engine/tests/objects.rs`; no view code pushes it.
- `Shape::ALL` is fourteen bounded primitives with described parameters;
  `place_object` puts one into the *active* layer as a node.
- `ClayDocument` holds one `mask` (a standalone `Mask::new`, not the engine's
  per-layer mask), one `symmetry: [bool; 3]`, one `armature`, one `lattice`.
  The engine and the `claycore` wrapper are already per-layer for the first
  three: `Document::add_mask(layer, cell_size)` exists in
  `crates/claycore/src/mask.rs`, `set_layer_mirror` takes a layer, armature
  nodes live in a layer.
- The renderer receives one concatenated buffer for all carried (voxel/mesh)
  geometry and one meshed surface for the union of all visible SDF layers.
- Engine history, once enabled, records every command — including
  `SetLayerVisibleCmd`; there is no pause. App undo walks engine depth with
  interleaving bookkeeping (`mesh_undo`, `crossing_undo`, `suppressed`).
- Layers compose by **hard union** (`clay/scene/tape.h:113`), so no live
  boolean between layers exists. `clay_item_volume_from_document` bakes a
  document's field into a volume item, and a hidden layer "contributes nothing
  to the field; showing it again restores the original field exactly".

## Goals / Non-Goals

**Goals:**

- One activation path: viewport click and stack click converge on the same
  command, so `active` and `selected` cannot disagree.
- A subtool boolean that is honest about being resolved rather than live, and
  is recoverable — one undo, operands kept.
- Per-layer sculpting state with no behavioural change for a single-layer
  document: the existing 1170 tests keep passing untouched.

**Non-Goals:**

- No change to `claycore-sys` or the vendored engine; `claycore` may gain thin
  wrappers over ABI calls it already links.
- No live layer-level boolean (ClayCore #321), no per-subtool undo histories,
  no subtool folders.
- No triangle-level attribution of the merged SDF surface to layers.

## Decisions

**1. Viewport activation goes through the object-pick pipeline, not beside
it.** The app already routes clicks to `ObjectViewModel::pick_at`, which is
cross-layer. Order: if the hit resolves to a placed object, keep today's
behaviour (select the object, offer its manipulator) *and* activate its layer;
otherwise activate the hit layer via `Command::SelectLayer`. One command
mutates activation, so stack and viewport stay one mechanism. Alternative
considered: a separate `select_at` path beside object picking — rejected,
because two pickers over one click is how `active` and `selected` diverged.

**2. Insertion composes existing calls; it is not a new engine path.** Insert
a primitive as a subtool = `add_layer(name, Sdf)` then `place_object(shape, …)`
in the layer just created, wrapped in one undo gesture (the same
`begin_target_drag`/`end_target_drag` grouping placement drags already use).
Insert a mesh = the existing `add_mesh_layer`. Copy a subtool = bake the source
to a volume (decision 3's machinery) into a fresh layer — an honest copy at a
stated resolution, and the same code path the boolean already needs. Instancing
would be cheaper and is the upstream ask (ClayCore #364); a copy is what can
be built today, so the interface says "copy".

**3. The subtool boolean bakes each operand with the others hidden.**
`clay_item_volume_from_document` samples the whole document field, and hiding a
layer removes its contribution *exactly* — so baking one subtool alone is: hide
every other layer, bake over the region, restore. This is the solo mechanism
(decision 4) reused, and it is why solo is built first. The operation is then:

1. Region = union of the two operands' `clay_layer_bounds`, padded by the band.
2. Bake operand A alone → volume item; bake operand B alone → volume item.
3. New SDF layer; add A with `CLAY_OP_ADD`, add B with the chosen op.
4. Hide (default) or remove (on request) the operands; select the result.

All of it inside one undo gesture so a single ⌘Z reverses the whole thing.
Voxel and mesh operands need no special case: the bake reads the evaluated
field, and a voxel or mesh layer contributes to it like any other. Resolution
defaults from the operands' own detail (`detail.rs` already prices this
vocabulary) and is the sculptor's to change; the cost is presented with the
same `Cost` type the conversion crossings use, which is why
`ObjectModel::mesh_operand_cost` is the precedent to follow rather than a new
one. Alternatives considered: per-node booleans inside one layer (that is the
existing placed-object feature and it cannot combine two *sculpted* forms);
waiting for #321 (blocks the feature on an engine release).

**4. Solo flips engine visibility and app undo hops the steps it created.**
There is no journal pause, and the SDF union surface cannot drop a layer any
other way than engine visibility. So solo issues `set_layer_visible`, records
the engine depths those commands produced, and undo/redo skip solo-created
depths using the same depth bookkeeping the mesh/crossing interleaving already
does. Saving while soloed writes the real visibility pattern, saves, then
re-applies the solo. The boolean's hide-and-restore uses this same recorded-
depth machinery, which is why they share an implementation.

**5. Per-layer state lives on `Layer`, restored on activation.** `mask`,
`symmetry` and `armature` move from `ClayDocument` to `Layer`. Symmetry: the
engine already stores per-layer mirror, so the app-side cache moves into
`Layer` and `set_active_layer` stops re-pointing it. Mask: switch to the
engine's per-layer `add_mask(layer, cell_size)`, which also puts masks in the
document's save path. Armature: the existing tuple becomes a field of the
layer holding the nodes. The cage stays a transient authoring gesture rather
than per-layer state — it is resolved on switch (apply or drop), because a
cage sized to one form has no meaning around another. Alternative considered:
side maps keyed by `LayerKey` beside `ClayDocument` — rejected; `Layer` is
where every other per-layer fact already lives.

**6. The active-subtool cue is two mechanisms behind one look.** Carried
geometry gains per-layer index ranges — `visible_mesh_geometry` already walks
layers in order — and the renderer issues one draw per range with a tint
uniform; subtool counts are small, so the extra draws are noise. The merged SDF
surface cannot be split per layer without attribution the engine does not
offer, so an active *SDF* subtool is cued by its bounds outline
(`clay_layer_bounds`), drawn the way the selected object's box already is. The
spec asks for "a consistent cue" rather than a tint to leave this split legal.
Alternative considered: re-meshing the active layer's field alone for a true
tint — a second full evaluation per frame, rejected on cost.

## Risks / Trade-offs

- [The boolean is destructive and users will expect ZBrush's live boolean] →
  operands kept and hidden by default, cost stated before it runs, one undo
  reverses it; the interface says the result is resolved. #321 upgrades this
  to live later without changing the vocabulary.
- [Bake resolution loses detail on a heavily sculpted operand] → resolution is
  chosen with the cost in view and defaults from the operands' own detail; the
  regression suite includes a boolean over a sculpted form, not just over two
  primitives.
- [Hide-bake-restore leaves the document wrong if it fails midway] → the
  visibility snapshot is restored on every exit path including the error path,
  and a regression test forces a failing bake and asserts visibility is what
  it was.
- [Undo hop cost] Skipping a solo-created step still calls
  `clay_document_undo`, and #210 means each hop dirties a whole layer →
  bounded by layer count; measured before a gate is set.
- [Engine mask semantics differ from the standalone mask] The per-layer mask
  freezes cell size against the layer where the standalone one used
  `VOXEL_SIZE` → the existing mask suites decide equivalence; a difference
  surfaces as a failing test rather than a silent change.
- [Sculptor rearm on click] Activation by viewport click makes
  `arm_mesh_sculptor` fire on a gesture that used to be free → measure switch
  latency; if it misses the budget, arm lazily on the first dab.
- [More voxel layers hit the duplicate-name refusal] → upstream ask filed
  (#365); until it lands, insertion derives unique default names.

## Open Questions

- ~~Whether the active-SDF-subtool bounds outline reads well when one subtool
  dominates the viewport.~~ **Answered: the outline stays.** The captures are
  `active-subtool-sdf-outline` (two forms in frame) and
  `active-subtool-sdf-dominant` (the camera in close on the active one), both
  from `crates/clayspace-app/tests/visual_active_subtool.rs`.

  With both forms in frame the box reads immediately: three faces of it stand
  around one sphere and not the other, and it says which without competing with
  either silhouette. Zoomed in far enough that the active subtool fills the
  viewport, the box degenerates — its edges leave the frame and what is left is
  two warm lines in the corners, which is noise rather than a cue.

  It stays anyway, for two reasons. The degenerate view is the one where the
  question is not being asked: the requirement is that the active subtool be
  distinguishable *from the other visible layers*, and at that zoom there is no
  other form on screen to confuse it with — which is also why the cue is
  suppressed outright while only one layer is visible. And the fallback is worse
  where it matters: the union surface is one mesh, so dimming it dims every
  visible SDF subtool together, which says nothing at all in the case the spec
  actually names — two SDF layers visible, the second activated. Dimming would
  trade a cue that is weak when zoomed in for no cue at all when zoomed out.
