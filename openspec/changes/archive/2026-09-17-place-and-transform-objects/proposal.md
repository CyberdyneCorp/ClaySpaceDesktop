## Why

Three of the four pieces of a solid-modelling workflow are already here, tested,
and wired to nothing that can use them.

**The booleans exist.** `Combine` carries fourteen of the engine's sixteen
operations with their blend profiles, their inversion rules and the seven that
mean nothing without a distance. Every one of them answers only *how the next
brush dab meets the surface*. There is no shape you can point at.

**The manipulator exists.** `clayspace-model/src/gizmo.rs` is a complete
move/turn/scale widget: three axis handles, the view-facing rotation ring, a
centre handle, drag-plane maths and Ctrl-snapping. It acts on lattice cage
control points and on nothing else, which `docs/features.md` already records
under *Not built yet* — "**A manipulator outside the cage.** Transforming a
whole layer with it — which is the other thing both references use a gizmo for
— has no route here yet."

**The engine's side is complete.** Thirty primitives against the four the
bridge wraps. `clay_layer_add_item` returns a node id; `clay_layer_set_transform`
retransforms that node in place; `clay_layer_set_op_blend` changes how it
combines after it is placed; `clay_layer_set_prim` swaps the shape and keeps
everything else; `clay_document_set_layer_transform` does the same for a whole
layer, and `clay_mesh_transform` for a mesh.

What is missing is the thing in the middle: **a placed object**. The
application knows about layers and about strokes, and there is nothing between
them for a gizmo to grab or a boolean to take as an operand. A sculptor cannot
put a cylinder through a form and move it until the hole is where they want it,
which is the first thing anyone coming from Nomad or Blender tries.

## What Changes

- **A scene holds objects.** Placing a primitive adds an addressable item to
  the active SDF layer and selects it. It stays addressable: this is a live
  operand, not a stamp. Move it a week later and the boolean follows.
- **Fourteen primitives are offered** — box, sphere, cylinder, cone, torus,
  capsule, ellipsoid and the rest of the bounded ones the engine carries, each
  with the parameters it takes. The two the engine calls
  unbounded — a plane and an infinite cylinder — are left out: they have no
  extent to draw a gizmo around and no bounds for the cache to work from.
- **An object carries its own boolean.** The combine operation and blend
  profile become properties of the placed object rather than of the stroke
  that is not being made, and both are editable after placement. Subtract a
  cylinder, decide it should be a groove, change it without replacing it.
- **A custom object can be an operand.** Using a mesh layer converts it on use,
  through the conversion panel that already computes and states what the
  crossing costs — surface movement of about half a cell, features thinner than
  a cell, sharp edges to a staircase. The mesh layer stays where it is; the
  operand is the converted copy.
- **The manipulator comes out of the cage.** It transforms a placed object, a
  whole layer, an imported mesh, and a curve's control points as a group.
  Sculpting strokes stay out of its reach: a stroke is a gesture that is over,
  and picking one back up is a different feature with a different question
  behind it.
- **Scale is uniform, and says so.** Every transform in the engine's ABI takes
  one `float scale` and not a vector. Scale mode therefore offers the centre
  handle and not three axis boxes, because three controls that silently do
  nothing is the failure mode this codebase keeps refusing.
- **A layer gains a transform.** Which is what makes "move this whole thing"
  reachable, and what the symmetry plane already follows: the engine's note is
  that a layer transform moves the mirror plane with the layer.
- **Placing, transforming and removing are undoable**, each as one step, and
  each survives save and reload — a node's transform is what the document
  format records.

## Capabilities

### New Capabilities
- `scene-objects`: what a placed object is, how one is inserted, selected,
  re-shaped and removed, and what the scene shows of the ones it holds.
- `object-transform`: the manipulator on something other than a cage — what it
  can grab, what each of its three modes does to each of those, and what it
  refuses.

### Modified Capabilities
- `sculpting-tools`: a combine operation stops being only a property of the
  next stroke. It is what a placed object *is*, editable after the fact, and
  the same fourteen operations serve both.
- `scene-and-layers`: a layer gains a transform of its own, and its contents
  stop being anonymous — the items inside an SDF layer become addressable,
  which is what selection and the gizmo both need.
- `representation-conversion`: a mesh layer offered as a boolean operand
  converts on use rather than refusing, stating the same costs the conversion
  panel states, and leaving the source layer where it is.

## Impact

- `claycore`: wrappers for the bounded primitives, `clay_layer_set_transform`,
  `clay_layer_set_op_blend`, `clay_layer_set_prim`, `clay_remove_node`,
  `clay_document_set_layer_transform` and `clay_mesh_transform`.
- `clayspace-engine`: node ids stop being tracked only for the snakehook, the
  armature and the live curve; an SDF layer's items become an addressable list
  with a selection, and `ClayDocument` gains place, retransform, re-op and
  remove.
- `clayspace-model`: an object and its selection as domain types; `GizmoDrag`
  learns what it is dragging, since a cage point and a whole object take a
  displacement differently.
- `clayspace-vm`: commands for insertion, selection, transform and removal, and
  the state the shelf and the options bar read to present them.
- `clayspace-view`: the manipulator drawn on an object rather than only on a
  cage, the primitive picker, and per-object hit testing in the viewport.
- Documentation: `docs/features.md`'s *Not built yet* entry for the manipulator
  and its *Deliberately absent* entry for mesh-surface booleans, both of which
  this change answers in part; `README.md`'s input table for the new gestures.
- Performance: the gate grows figures for placement, retransform and the
  re-evaluation a moved operand forces — a live boolean is re-evaluated on
  every drag frame, which is the one thing here that could be too slow to use.
- No engine-version floor change: every entry point named above is in the
  pinned 0.39.0 ABI.
