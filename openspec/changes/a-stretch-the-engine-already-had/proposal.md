# A placed object stretches, because the engine has let it since 0.54.0

## Why

The interface said, in four places and a requirement, that a scale is one
number:

> Every transform in the engine's interface takes a single scale factor rather
> than one per axis.

That has not been true since ClayCore 0.54.0, and this application is pinned to
0.60.0. The C ABI carries `clay_item_set_scale_nonuniform` and
`clay_layer_set_transform_nonuniform`, both documented at length — what they
mean, what they cost, and why they are not on the descriptor struct — and
`claycore` had bound neither. The belief was written down in `Transform`, in
`SceneObject`, in `GizmoHandle::combined`, in `GizmoDrag::factor` and in the
specification, and nothing ever went back to check it.

The visible consequence: a capsule could not be squashed into a slot, a
cylinder could not become an oval bolt hole, and the manipulator's three scale
boxes were drawn on a deformation cage and hidden on every placed object.

## What Changes

- **`claycore` binds the per-axis node transform.**
- **A placed object carries a scale per axis**, through the domain, the
  ViewModel and the side-car.
- **The manipulator offers its three boxes on a placed object.** A box on an
  axis stretches that axis; the centre handle takes all three.
- **A whole subtool still scales uniformly**, because the engine's *layer*
  transform does take one factor — `clay_document_set_layer_transform` has no
  per-axis form. The boxes are offered exactly where they can be applied.
- **The readout shows three factors where they differ** and one where they do
  not, so an evenly scaled object still reads as one number.

## Decisions worth stating

**The per-axis call is used for every object transform, not only stretched
ones.** The ABI does not do partial updates: each of the two calls writes the
*whole* transform, so the uniform one applied to a node carrying a stretch
collapses it. One call for both means a move can never quietly unsquash what it
moves. A uniform value costs nothing — the engine is explicit that `(1, 1, 1)`
and any other uniform triple keep the field exact and compile to identical tape.

**What a stretch costs is not what one would guess.** The field stays
1-Lipschitz, so the safe step scale is unchanged and a marcher takes the steps
it always did. What is lost is *exactness*: the value becomes a bound on the
distance rather than the distance, short by at most the ratio of the largest
axis to the smallest, and never an overestimate. That matters to a consumer
reading the value *as* a distance and to nothing else.

**The side-car grows at its end, not in the middle.** It is a positional text
format. The first scale component stands where the single uniform one stood and
the other two are appended after the counted run of parameters, so a build that
predates this reads a stretched object as evenly scaled rather than failing to
read the row — a degradation, not a corruption. Growing in the middle would
have shifted the parameter count and made every row unreadable.

**Two stale answers went.** `GizmoHandle::all_for_transform` and the
ViewModel's `handles()` both answered "which handles does this target carry",
beside `GizmoHandle::combined`, which is the one the picture and the hit test
both read. Both existed only to state the belief this change disproves, and
neither had a caller outside its own tests. The ViewModel now answers
`per_axis_scale()` — the question the composition root actually asks — and the
composition root stopped deciding it for itself.

## Out of scope

- **Three rotation values.** The engine stores a quaternion and its ABI speaks
  an axis and an angle, which it is careful to call *a* representative rather
  than the representation. Decomposing into three Euler angles picks one of
  several answers, and a readout whose numbers changed when nothing had moved
  would be worse than one that shows what the document holds. This one really
  is a thing the domain does not have.
- **A non-uniform layer transform.** The engine has none.
