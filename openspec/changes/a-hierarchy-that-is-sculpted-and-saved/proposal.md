# A hierarchy that is sculpted, and that survives being saved

## Why

`a-hierarchy-the-domain-can-describe` put the fourth representation in the
domain and said, in as many words, that nothing sculpts one: the crossing, the
stroke, the boolean operand and `add_layer` all refused in the adapter, because
no `Layer` held a `clay_multires`. This is the change that makes a hierarchy a
thing a sculptor works on.

Three of the four problems it solves are ordinary — hold the handle, route the
stroke, wire the crossings — and one is not. **A `.clayspace` does not carry a
hierarchy.** ClayCore v0.78.0 states it first among its known limits and the C
header repeats it: a `clay_multires` is a free-standing owning handle that took
a copy of the cage on the way in, `clay_multires_serialize` is the only route
the sculpt has to disk, and *where those bytes go is the host's decision*. The
repository already holds that as a tripwire —
`claycore/tests/multires_document.rs` saves the same document either side of a
dab on the hierarchy built from its cage and gets **812 bytes, byte for byte
identical**, while the dab takes the finest level's relief from 0.000 to 0.883.
So the side-car is not plumbing. It is the feature.

Two more consequences follow from the same ownership boundary, and both would
be silent if they were got wrong:

* **There is no undo record.** clay.h says so twice, unprompted, of the
  resolved stroke and of the layered transaction alike: *"the record itself
  does not cross this ABI yet, which is stated here rather than left to be
  discovered."* A host that wants a layered gesture in an undo stack is pointed
  at pyclay or the C++ `SculptLayerDelta`.
* **A hierarchy put back is a new handle**, whose revisions restart at one. A
  document that dabs, undoes and redoes walks its evaluated revision
  1 → 3 → 1 → 1: the same number on either side of the redo, over two different
  surfaces. Anything watching the engine's counter alone concludes nothing
  happened, and the redo appears not to have taken.

## What Changes

- **A `Layer` holds a hierarchy beside its mesh layer.** A hierarchy row is two
  things at once: a real mesh layer in the `.clayspace`, holding the cage, and
  a `clay_multires` held beside the document. The layer is where its name, its
  place in the stack, its transform, its mask and its save come from; the
  hierarchy is where its levels and its detail do.
- **The two crossings work.** `MeshToMultires` takes the layer's own mesh as
  the cage — refusing rather than repairing, and naming which fault — and
  `MultiresToMesh` bakes the display level back out. Neither samples anything.
- **A stroke reaches a hierarchy**, through the same descriptor and the same
  preset a mesh stroke uses, because the engine says it is the same code. The
  assembly is extracted so the two cannot drift.
- **A hierarchy is drawn from its display level**, cached against a revision,
  and picked against the triangles that were drawn rather than against the cage
  underneath them.
- **The sculpt is saved beside the document**, in one file rewritten whole,
  priced by `clay_multires_preflight_encode` before it allocates. A failure to
  write it **fails the save**, which is the opposite of what the object table
  beside it does and is deliberate: the object table is bookkeeping and this is
  the work.
- **A document whose side-car has gone opens as the cage it holds.** Not
  refused, and not opened while going on calling the row a hierarchy — the row
  becomes the mesh layer it demonstrably now is, which the layer stack, the
  workspace bar and the inspector all draw differently, and the loss is named
  in the diagnostics report.
- **A gesture is one undo, and it is exact.** The history holds the hierarchy's
  own serialized bytes, in the same stack a mesh gesture goes into so the two
  order against each other, bounded by bytes so an unbounded record cannot eat
  a session.
- **A level is priced and refused.** `clay_multires_preflight_add_level` states
  the **peak** during the build rather than what remains after it, the refusal
  arrives as a `Refusal` a sculptor can read, and the hierarchy is exactly as
  deep as it was afterwards — build-then-publish.

## What this deliberately does not do

- **The pass stack.** `MultiresSculptLayerOp` is modelled and nothing here
  applies one; a stroke goes into the form under the passes, which is what an
  empty stack means. Wiring it wants the layer-stack UI that draws both stacks
  from one row widget, which is its own change.
- **Exporting a hierarchy exports its cage.** `mesh_combined` reaches the
  layer, and the layer holds the cage. Keeping the layer's triangles in step
  with the display level means a wholesale geometry replacement — one engine
  undo entry each — on every gesture or every save, and either puts a document
  edit inside something a sculptor did not ask to be an edit. The route that
  exports a sculpt is the crossing that bakes a level out, which is one step
  and says what it gives up.
- **A hierarchy is not a boolean operand.** Routing it through the mesh arm
  would sample the *cage* — a boolean against a subtool the sculptor can see,
  using geometry they cannot.
- **No benchmark figure.** A hierarchy is a new bench family with no committed
  baselines, and the A/B against the previous engine pin cannot compare a
  figure that exists on one side only. It needs its own change after the
  measurement phase.
