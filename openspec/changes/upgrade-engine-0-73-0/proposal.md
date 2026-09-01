# Move the engine pin to ClayCore v0.73.0

## Why

The pinned engine is v0.60.0. v0.73.0 is thirteen minor versions ahead in one
tag — 0.61.0 through 0.73.0 — and two of the things it fixes are defects this
repository measured, filed and then wrote tests to hold open. Both of those
tests fired on the move, which is the best possible reason to make it.

**The pin moves cleanly.** The C ABI gained 52 entry points with nothing
removed, no signature changed and no struct re-laid out. Two descriptors grew
behind the `struct_size` they already negotiate, and this workspace writes that
size from `size_of` of the compiled type rather than by hand, so the growth is
absorbed without a line changing. The scene and `.clayspace` formats do not
move — both stay at minor 15 — so a document written by either build is the
same bytes for the same content. The whole workspace compiled against the new
engine with no source change at all.

**A mask now protects a region from an operation, and that is the headline.**
`clay_item_set_gate` is the entry point that makes masking protect a surface
from *any* operation rather than only from a brush, and it has been in this
codebase's wrapper — matching its documented contract — doing nothing, since
v0.39.0. Measured then with a mask sampling 1.0 at a cut's own centre and
65,752 cells painted, a subtraction ate the protected region at every width and
threshold tried, and never refused. So `stroke_sdf` removed its call, and
`claycore/tests/mask_gate.rs` was written as a tripwire that fails the day the
engine starts honouring the gate.

It fired. The cause was never the threshold or the width: the gate was placed
by the transform of *the item it protects*, while the mask it measures is
stored in world units, so a cut with a placement carried its own protection
away from where the mask was painted. At the identity nothing moves, which is
why no fixture upstream caught it. That is CyberdyneCorp/ClayCore#394, fixed in
ABI 0.67.0, and the header now says outright that "the gate is in world space,
and does not travel with the item".

So the call goes back, and the requirement *Masks gate operations, not only
brushes* — written for `make-representations-first-class` and unmet since — is
met. Measured through the application: an unmasked subtracting stroke takes the
centre of the starting form from 1.0 to 0.825, and a masked one leaves it at
1.0.

**DynaMesh, because a mesh layer had no repair at all.** 0.63.0 added
`mesh::voxel_remesh` and 0.64.0 put it inside the document with an undo record.
It is the operation a sculptor reaches for when a form has been pulled
somewhere its triangles cannot follow: overlapping shells fuse,
self-intersections resolve, stretched triangles disappear and the density comes
out uniform. A field layer could already be collapsed when it steepened; a mesh
layer could be closed and filled but never rebuilt.

**The alpha tripwire did *not* fire, and that is worth stating.** #392 is in
this release and is about a *placed* item's stamp arriving in the wrong frame.
The defect this repository measured is a different one: `clay_layer_apply_stroke`
still does not resolve a template's deformer chain into each stamp's frame, so
`a_stroke_does_not_carry_the_chain_into_each_stamp` still passes and
`AlphaSupport` still refuses an alpha on an SDF stroke, for the reason it
always gave.

## What changes

- The submodule pin, `EXPECTED_ABI`, and the documentation that states the
  engine version.
- `stroke_sdf` gates its stroke template with the active layer's mask. Set on
  the template and correct for every stamp, because the gate is in world space
  and does not travel with the item — unlike the alpha, which is in the item's
  own frame and is why a stroke cannot carry one. The two tripwire files are
  turned around to hold the protection rather than its absence.
- A mesh layer can be rebuilt through a voxel field, from the layer stack: a
  resolution the sculptor drives, three switches, and an outcome that states
  what the rebuild destroyed. Bound in `claycore`, offered through
  `SceneModel::remesh_layer`, and drawn beside the layer stack where the field
  layer's *Optimizar* row already is.

## What does not change

The rest of the release's 52 entry points are not taken up here. The adaptive
surface, brush presets across the ABI, the live voxel drag, the drag preview as
an ordinary document and `degradation` in place of `advises_consolidation` each
retire something real in this codebase and each is its own change with its own
measurements.

## What we found that upstream has not

**A mesh layer's geometry revision does not move when history replaces its
triangles.** `clay_document_mesh_layer_revision` is documented as bumped "every
time a layer's triangles are replaced wholesale", and the reason given for it
existing is the cache a wholesale replacement invalidates — an adjacency, a
BVH, a live sculptor, "wrong in a way nothing else detects". Measured on
0.73.0: a layer attached at revision 1 and rebuilt to revision 2 comes back to
its original 119,100 triangles under undo and to the rebuilt 37,752 under redo,
at revision 2 throughout. The one moment the number was added for is the one
moment it is silent.

It is not theoretical here. With the number as the only signal, a sculptor who
rebuilds, dislikes the result, undoes and keeps working gets
`clay_mesh_sculptor_apply_stroke: the mesh changed its vertex or index count
under this sculptor` on their next stroke — measured, by disabling this
change's own answer and watching the test fail with exactly that. So
`ClayDocument` records the engine depth each rebuild sits at and drops the
sculptor when history stands on either side of one, and
`claycore/tests/voxel_remesh.rs` holds the gap as an equality that fails the
day the engine closes it.
