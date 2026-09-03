# A whole subtool stretches, and the engine says where it stands

## Why

`a-stretch-the-engine-already-had` ends with a sentence in its "Out of scope":

> **A non-uniform layer transform.** The engine has none.

It has one. `clay_document_set_layer_transform_nonuniform` landed in ClayCore
ABI 0.74.0 as issue #373, and this workspace is now pinned to v0.78.0. So the
half of that change which was a real limitation — a placed object stretched, a
whole subtool did not, and the manipulator drew two different widgets depending
on which it stood on — is a limitation no longer.

The same ABI minor closes a second gap, and it is the longer-standing of the
two. `clay_document_layer_transform` and
`clay_document_layer_transform_nonuniform` answer **where a layer is**, which
the boundary could not previously be asked at all — the ABI set a layer
transform and would not read one back, and had not since layers existed. This
application had built two mechanisms on that absence, and one of them was
hiding a defect: `ClayDocument::from_file` had no way to learn where a saved
layer stood, so **every reopened document believed every subtool was at the
origin, unturned and unscaled**. On a field layer the engine still evaluated
the tape where the layer really was, so the form was drawn correctly and
everything the host derives from the placement was wrong — the whole-subtool
manipulator sat in empty space, a mirrored dab reflected through the world's
plane rather than the layer's, and a mask painted in world coordinates missed
the cells it was meant to protect. On a carried mesh or a grid, where the host
applies the placement itself, the subtool came back at the origin.

## What Changes

- **The manipulator offers its three scale boxes on a whole subtool.** One
  widget with the same handles wherever it stands, which is what ZBrush's
  Gizmo 3D and Maya's scale tool both are.
- **Every layer transform is written through the per-axis call**, including the
  ones that carry a single factor, so a move can never quietly unsquash what it
  moves.
- **`Transform::into_world` / `into_local` stretch per axis**, and compose the
  scale *innermost* — `world = rotation · diag(scale) · point`, the order the
  engine composes a layer in.
- **A world radius carried into a subtool's own frame is divided by the
  largest factor**, not by the mean.
- **A stretched subtool is refused the deformation cage in words.**
- **Where a layer stands is read back from the engine** — on open, and after
  every history move. The host-side snapshot table that reconstructed it is
  gone.
- **A `.clayspace` this build writes is at container minor 16**, and the
  constant that says so is checkable rather than asserted.

## Decisions worth stating

**This build writes minor 16, and it could not write 15 if it wanted to.** The
release notes' upgrade item 1 says a host that exchanges documents with an
older build should write at minor 15, where the per-axis layer scale is dropped
and a squashed layer comes back at the identity triple (1, 1, 1) — the document
opens, and the loss is visible rather than fatal. **That advice is not
reachable across this ABI.** The minor to write at is a parameter on the C++
`scene::serialize_document`; it is not on `io::save_clayspace`, which has no
such parameter at all, and it is not on `clay_document_save`, which takes a
path and nothing else.

It is also the choice this repository would have made. It has followed the
engine's current minor through 7, 8, 11, 14 and 15 — every one of them the same
shape, a field inside a back-to-back record — and it exchanges documents with
no older build. What minor 16 costs is that a document written now is *refused*
by a build older than v0.78.0 rather than misread, which is the direction the
format is designed to fail in.

So the decision is named rather than left implicit: `claycore::Document::FORMAT`
carries it with the reasoning, `Document::format_of` reads what a file actually
says so the constant can be checked against a written document, a test reads
`kClaySpaceMinor` and `kSceneMinor` out of the pinned engine's own headers and
fails when either moves past it, and the diagnostics report carries the number
so "it will not open on the other machine" has an answer a person can quote.

**A world radius takes the largest factor, and that is the engine's rule rather
than a choice made here.** Five sites divide a brush radius by a layer's scale
to reach the layer's own coordinates, and a subtool squashed three to one has
no single divisor. The header settles it: an evaluated distance is multiplied
back by the *smallest* component so the field never overestimates and stays
1-Lipschitz, and "a world RADIUS mapped inward is divided by the LARGEST
component instead — the dual, so a gesture never reaches outside the region it
named." The mean is the obvious answer and is the wrong one: on a squashed
subtool it lets the brush reach past its own ring on the wide axis, which is a
dab landing where the sculptor was not pointing.

**A stretched subtool loses its deformation cage, and is told so.**
`clay_layer_lattice_gizmo` returns no warps at all for a layer carrying a
per-axis scale — a cage records its item-to-cage placement as a rigid transform
and on a squashed layer the map it needs is a general affine one, so placing a
cage through the narrower record would warp every item in a space it does not
occupy, silently. The engine refuses rather than approximating. Without a
refusal of our own the sculptor would have been told "the cage reached nothing
in this layer", which is true and names nothing they can undo.

**The host-side snapshot of every layer's placement is gone, and the cache is
not.** `layer_states` recorded the whole stack against the engine's undo depth
from six call sites, purely so that an undo could be followed — the engine
reverted a layer transform and could not say that it had.
`clay_document_layer_transform_nonuniform` says it, so `resync_layer_transforms`
asks. `Layer::transform` stays as a *cache* of that answer, because
`carried_placement` reads it per stroke segment and a round trip through the
ABI per segment buys nothing. The object table stays for the reason it was
built: a *node's* transform still has no reader.

**The per-axis reader, always.** The single-factor reader refuses a layer
carrying three different factors with `CLAY_ERROR_INVALID_ARGUMENT` rather than
averaging them away, so a squashed subtool answers nothing at all through it.
The per-axis one reports the product of the layer's two scales, so a uniformly
placed layer reads `(s, s, s)` and one manipulator never has to branch.

## Out of scope

- **Retiring the object side-car.** #373 gives *layer* transform readback. A
  node's transform, parameters and op-blend still have no getter, which is what
  `clayspace_engine::objects` exists for.
- **A readout of a subtool's three factors.** The viewport's transform panel is
  drawn for a placed object alone, because "a cage's target is a set of control
  points and a layer's is everything it holds". Widening it is a presentation
  change of its own.
- **Writing at an older minor.** Not reachable across this ABI; see above.
