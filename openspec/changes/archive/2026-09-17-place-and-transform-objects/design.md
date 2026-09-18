## Context

See proposal.md — Why. What matters for the approach, and what was checked in
the engine's own header before any of it was decided:

- **The engine addresses items.** `clay_layer_add_item` returns a
  `clay_node_id`; `clay_layer_set_transform(layer, node, position, axis, angle,
  scale)` retransforms it; `clay_layer_set_op_blend` changes how it combines;
  `clay_layer_set_prim` swaps the shape and keeps "its deformers, repetition,
  profile and stroke"; `clay_remove_node` takes it away.
- **A layer's contents can be walked.** `clay_layer_node_count` /
  `clay_layer_node_at` enumerate, and `clay_layer_node_prim` says what each one
  is.
- **Picking already answers with a node.** `clay_raycast_attributed` reports
  "the layer and the item whose field is closest at the hit point, so a
  subtract item is attributed the surface it carved" — clicking the wall of a
  hole selects the cylinder that cut it. Its own note is that this is not the
  cheap path: it compiles the document, then one tape per layer and one per
  candidate item.
- **The dirty region of a move is computable.**
  `clay_layer_node_influence_bound` gives one node's box, and reports
  `*out_infinite` where there is no finite one. The header is precise about
  when that happens: "a non-local op (intersect, the spatial morphs) anywhere
  in the subtree, an infinite grid repeat, or an unbounded primitive (a plane,
  an infinite cylinder)". Two primitives, and — the part that matters here —
  **one of the operations we offer**.
- **Every transform in the ABI takes one `float scale`.** Not a vector. There
  is no per-axis scale for an item, a layer or a mesh.
- The application currently tracks `NodeId` in three places only — the live
  snakehook, the armature and the curve being authored. There is no general
  notion of an item.
- `clayspace-model/src/gizmo.rs` is complete and acts on a `Vec<[f32; 3]>` of
  cage points. Its drag resolution, ring maths, snapping and handle model are
  all reusable; what is not is the assumption that a target is a set of points.

## Goals / Non-Goals

**Goals:**

- A boolean workflow a sculptor from Nomad or Blender recognises: place, aim,
  combine, and come back to it later.
- One manipulator, four kinds of target, and the same rules on all of them.
- No control that silently does nothing — the standing rule here, and the
  reason scale is uniform rather than pretending otherwise.
- Objecthood that survives save and reload without a side-car file.

**Non-Goals:**

- Non-uniform scale. The engine has no route to it for a transform; the cage
  is what shapes a form unevenly and it is already built.
- Picking a sculpting stroke back up. `clay_layer_stroke_points` reads a placed
  guide back and this change does not use it — see the refusal in the spec.
- Mesh-surface booleans. A mesh still cannot compose; what changes is that the
  conversion is offered where the sculptor meets the problem.
- A general node tree in the interface. A worked layer holds hundreds of
  strokes and showing them as rows would be a worse scene panel than none.

## Decisions

### Objecthood is recorded, not derived

**This decision was reversed during implementation.** It read "an item is an
object if its primitive is not one of the three the application makes for other
reasons", which would have let an object survive a reopen with nothing extra
written to the document.

It does not hold. An SDF stamping stroke places `Item::sphere` per stamp, so a
worked layer is full of sphere items nobody placed, and the rule offers a row
for each — the exact thing the scene-and-layers requirement forbids in as many
words. `a_sculpting_stroke_is_not_an_object` is what caught it and what holds
it fixed.

So objecthood is recorded in the table that the readback gap already forces the
application to keep and save. That is not an extra cost: the table exists
either way, and it now answers one more question it is already the only thing
able to answer. What is lost is the graceful reading of a document written
elsewhere — without the side-car, its objects list as nothing rather than as
themselves. Which is the safe direction to fail in: the derived rule would have
listed a hundred stamps as objects, and this lists none.

The starting form is still an object, now by decision rather than by accident:
`add_starting_sphere` records it. It always was a placed sphere, and a sculptor
who wants to make the thing they are working on bigger should be able to select
it and say so.

### The application mirrors the state, because the engine will not say

**This decision was reversed during implementation.** It read "the engine holds
the state; the application holds an index", on the reasoning that mirroring an
object's transform is the shape that produces two answers to "where is the
cylinder" the first time an undo runs. That is still true. It is also not
available: the ABI sets a node's transform, parameters and operation and reads
none of them back.

What can be read is `clay_layer_node_prim` — which primitive a node is — and
the header states the reload model it belongs to: "ask what the node is, then
call the reader that applies". Typed readers exist for an armature and for a
stroke's points. There is none for a plain item, and no host-data channel in
the document to hide one in.

So the application keeps a table: node id to shape, parameters, transform and
combine settings. Two things follow, and both are worked rather than hoped:

- **It is saved beside the document**, keyed by node id. That key is
  load-bearing, so it is checked rather than assumed:
  `a_node_id_survives_a_save_and_a_reopen` places two objects, removes the
  first — leaving a gap in the id space, which is the case a naive scheme gets
  wrong — saves, reopens, and asserts the survivor's id is unchanged and the
  removed one has not come back.
- **Undo is paired.** An object edit pushes its inverse onto a stack the
  application pops in lockstep with the engine's undo, because the engine
  reverts the transform and cannot tell the table it did.

`clay_layer_node_influence_bound` is a partial getter and is used as one: the
centre of a node's box comes from the engine, so it is right even when the
table is wrong, and it is what the manipulator is *drawn* on. It cannot
resolve a drag — `set_node_transform` takes an absolute transform, so applying
a delta needs the current rotation and scale, which only the table has.

The getters are worth having and this application is not the place to invent
them: `clay_layer_node_transform`, `clay_layer_node_params` and
`clay_layer_node_op_blend` are filed upstream, a repro test holds the gap the
way `claycore_repros.rs` holds the others, and this table comes out when they
land.

### A drag is one undo group, and one refill per frame

A drag brackets `begin_undo_group` / `end_undo_group` around the whole gesture,
which is what makes it one entry — the same thing `convert_layer` does for the
several edits a crossing makes, and for the same reason.

Each frame of the drag sets the transform and refills the union of the node's
influence bound *before* and *after* the move. Both boxes, because vacating a
region is as much a change as arriving in one: refilling only the destination
leaves the surface the object used to cut still cut.

A node with no finite influence reports `*out_infinite`, and the honest
response the header names is to dirty everything. Excluding the two unbounded
primitives does not make this unreachable: **an object placed with `Intersect`
has no finite bound whatever its shape**, because a non-local op anywhere in
the subtree removes one. So the wide path is a normal path, not an edge case,
and it is handled by dirtying the layer rather than by assuming a box that is
not there.

### Picking is a click, never a hover

`clay_raycast_attributed` compiles the document and a tape per candidate item.
That is affordable when a sculptor clicks and unaffordable at sixty hertz, so
selection happens on press and hover highlights nothing. If a hover affordance
is wanted later, the cheap route is the manipulator's own handles, which are
screen-space geometry the viewport already owns.

### The manipulator learns what it is dragging

`GizmoDrag::apply` currently maps a point to a point, which is the whole of
what a cage needs. A target becomes one of four things instead:

- **Object**: `clay_layer_set_transform` on its node.
- **Layer**: `clay_document_set_layer_transform`.
- **Mesh layer**: the layer transform, which the engine composes for a mesh
  the same way — and `clay_mesh_transform` only where a bake needs the moved
  vertices.
- **Curve points**: the existing point-set path, extended with turn and scale
  about the selection's middle. Move already exists as `drag_curve`.

The maths stays in `clayspace-model`: a drag resolves to a position, an
axis-angle and a scale, and which of those the target accepts is the target's
business.

Alternative considered: a transform matrix as the common currency. Rejected —
the ABI takes position, axis, angle and a scalar, and composing a matrix here
only to decompose it at the boundary invents a rotation representation the
engine would have to be reconciled with.

### Scale mode presents what the target can do

`GizmoHandle::all_for(mode)` gains the target as a second argument. For a cage
in scale mode it answers as it does today, three axis boxes and a centre. For
an object, a layer or a mesh it answers with the centre alone.

### A converted operand is a layer, not a hidden copy

Using a mesh as an operand runs the existing conversion, which "produces a new
layer rather than replacing one". The converted layer is visible in the stack
and can be removed like any other. It is not folded invisibly into the target
layer, because a sculptor who wonders where the operand went should be able to
find it, and because the conversion's own requirement already says a crossing
adds a layer.

### The live budget borrows the region-based tools' answer

If a drag frame's refill exceeds what keeps the viewport interactive, the
object is drawn moving against the last completed surface and the surface
settles when the pointer comes up. This is the shape the region-based brushes
already have — "they land when it comes up" — so it is a behaviour the
application has and a sculptor here has already met, rather than a new kind of
lag.

## Risks / Trade-offs

- **An intersecting object dirties the whole layer on every drag frame.** Not
  a shortcoming of this design — the engine states that a non-local op has no
  finite influence bound — but it means one of the fourteen operations is
  categorically more expensive to drag than the other thirteen. → Measured
  separately as `object.drag_frame_intersect`, and the settle-on-release path
  below is what carries it where it is too slow. Worth knowing before someone
  reports "the gizmo is fine except sometimes".
- **A live boolean is re-evaluated on every drag frame**, and that is the one
  thing here that could be too slow to be worth having. → Measured before it
  is tuned: the performance gate grows `object.place`, `object.retransform`
  and `object.drag_frame` figures, and the settle-on-release path above is the
  fallback the design already commits to rather than discovers.
- **Picking compiles the document.** → On press only, never on hover, and
  measured as `object.pick` so it cannot quietly become the thing that makes
  selection feel slow.
- **The starting form becomes deletable.** A sculptor can now select and remove
  the sphere the application opened on, leaving an empty document. → That is
  what removing an object means and it is undoable; the alternative is a
  special case that would have to be explained.
- **No non-uniform scale**, which is the first thing anyone will ask for after
  moving a cube. → The interface offers the cage for that, and says so where
  the scale handle is. Inventing one out of a lattice deformer per object was
  considered and is a different feature: it would make every object carry a
  deformer chain whose interaction with the boolean is its own question.
- **A document without its side-car shows no objects.** One written by another
  host, or one whose side-car was lost, still opens and still sculpts; its
  placed shapes are simply not offered as objects. → The safe direction: the
  rule this replaced would have listed every stroke stamp as an object instead.
- **A placed object is mirrored.** The application mirrors its layers in X "as
  the design asks for", and an object is an item in a layer like any other, so
  one placed off the plane cuts on both sides. That is almost certainly what a
  sculptor wants and it is not obvious from anything in the interface. → Held
  by `a_placed_object_is_mirrored_like_everything_else`, and it is why the
  manipulator cannot take an object's position from the engine's influence
  bound: that box covers both copies and centres between them. An object placed
  at 0.9 reported its position as the origin until this was found.
- **The object table is a second source of truth**, which the original design
  named as the thing to avoid and the ABI made unavoidable. → Bounded as
  tightly as it can be: it holds only what cannot be read back, the manipulator
  is drawn from the engine's own influence bound rather than from the table,
  and the paired undo is tested against the engine's history rather than
  assumed to match it. It is deleted, not adapted, when the getters land.
- **Two ways to move a layer** — the manipulator and whatever numeric control
  the panel offers. → They address the same engine value and the spec requires
  they agree; a test holds it.

## Open Questions

- Whether a placed object should be offered on a **voxel** layer by
  rasterizing it, rather than refused. The engine has
  `clay_voxel_rasterize`, so it is reachable; whether a sculptor wants a live
  operand on a grid — where nothing else is live — is a question about the
  workflow rather than the mechanism, and it can be answered after the SDF case
  is in someone's hands. The spec refuses it for now and says why.
