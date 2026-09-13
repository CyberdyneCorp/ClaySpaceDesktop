# Move the engine pin to ClayCore v0.113.0

## Why

The pinned engine is v0.84.0. v0.113.0 is **twenty-nine minors ahead in one
tag** — 0.85.0 through 0.113.0 — and it carries two releases' worth of content
that was never pinned here: v0.97.0, and v0.103.0, which was tagged and never
published.

**The pin moves with no source change at all.** Nothing was removed from the C
ABI across the whole span, no signature changed, and no struct was re-laid out:
every one grew by appending behind the `struct_size` this workspace writes from
`size_of`. The entire workspace compiled against v0.113.0 before a line of it
was touched, which is what makes everything below *adoption* rather than
*repair*.

**What it buys a sculptor arrives before any of our code does.** Picking on a
worked form, measured through `benchmarks/probes/move_dab_cost.rs` on one
machine, x-mirror, v0.84.0 against v0.113.0:

| dab | 256 raycasts | `safe_step_scale` |
|---|---|---|
| 4 | 9.60 ms → **3.90 ms** | 0.028932 → 0.036169 |
| 7 | 39.30 ms → **19.35 ms** | 0.002030 → 0.003000 |
| 8 | 35.73 ms → **18.56 ms** | 0.000837 → 0.001308 |

Roughly halved, and the declared Lipschitz bound about 1.5x less pessimistic,
from ClayCore #541 and #542 — with nothing of ours involved.

**Four constants move, and each was held by a test that fired the moment the
pin did.** `EXPECTED_ABI` to 0.113; `Document::FORMAT` to container minor 19;
the paired assertion that a file this build writes says the minor it claims;
and the error vocabulary, which gained a tenth code.

The container minor moved in **two steps inside v0.97.0** — 18 at ABI 0.87.0
and 19 at 0.88.0 — which is why "no format change" is true of the v0.113.0 span
and false of this jump. A document written now is refused by a build predating
v0.113.0 rather than misread, which is the direction the format is designed to
fail in, and this workspace exchanges documents with no older build.

## What the release lets us delete

Three things here existed only to work around engine defects that v0.113.0
fixes. Each is removed rather than kept beside its replacement.

**The whole-document re-mesh on stroke release.** It hid the brick mesher's
sliver triangles — 2,297 near-zero-area triangles in 83,464, whose face normals
are cross products of near-parallel edges and shade black. ClayCore #549 fixed
them at the source. A whole-field mesh has no region to cull against, so it
evaluates every grab on the layer for every sample and its cost tracks the
*document* rather than the edit: 2.8 ms at one dab against 26.2 ms at 48, where
the per-brick path goes 4.2 ms to 6.3 ms over the same span.

**And the second re-mesh nobody had counted.** A whole-document mesh cannot be
patched incrementally — the store held one giant key and the brick keys the
engine reports dirty do not address it — so a flag made the *next* sync throw it
away and rebuild every brick anyway. The field was meshed **twice per stroke**,
once whole on release and once per brick on the next edit. Both are gone, along
with the flag that scheduled the second.

**The `Rebuild` record.** `clay_document_mesh_layer_revision` is documented as
bumped whenever a layer's triangles are replaced wholesale, for the sake of the
cache that invalidates — and it was not bumped by history moving over a rebuild.
So the one moment the number existed for was the one moment it was silent, and
this workspace carried a list of rebuilds by engine depth to recognise a history
step across one. Fixed; the record and half of `settle_geometry_revisions` go.

**And the reason coarse-during-drag was refused.** Level 1 declined gradient
normals rather than downgrading them, so a coarse surface was face-shaded by
construction — and face normals on a coarse lattice measure up to 84.78 degrees
off the field. ClayCore #550 answers `CLAY_NORMAL_GRADIENT` at a level. The
coarse path asks for gradients now. Whether to *draw* coarse during a drag is
reopened rather than decided here.

## What the release lets us fix

**An export could be non-manifold and nothing said so.** `export_mesh` called
`mesh_combined` and then `save`, and nothing between them looked at the result,
while `Mesh::validate` sat bound with no caller on that path at all. That is the
class of defect that breaks a slicer or a boolean engine while a viewport shows
nothing wrong.

It is reachable on the export panel's own default: tick decimate, the slider
lands on 0.5, and the watertight mesher — the one carrying no caveat, and
therefore promising a 2-manifold — returns one that is not. ClayCore reproduced
it and filed #575, where **every avenue is refuted by measurement**: retry flags
return a byte-identical mesh, perturbing the input gives 0, 31 and 58 pinched
ratios of 76 from three meshes of one sphere, extending #549's guard to the tape
path nearly doubles it, and repair would decide the model's genus by a
floating-point tie-break. Detect-and-report is the only sound response available
from outside the simplifier.

So the application reports it. Not with `clay_mesh_validate`, which answers two
bits and drops the nine other quantities the same pass computed — six pinched
edges in ninety thousand triangles is a shippable file and six thousand is a
ruined one, and a sculptor told only "not manifold" cannot tell which they have.

## What the release does not explain

**A field's Move was invisible until the pointer came up**, and that is
pre-existing, ours, and older than any of this. `stamps_between_segments` asked
the *representation* before it asked whether the gesture *replays*, so every
field gesture waited three stamps' worth of travel — 1.03 world units at the
default flow, most of the way across a unit sphere — and an ordinary drag ended
before one segment fired. The live Move transaction that `arm_live_move` opens
for exactly this purpose was never fed a segment to preview.

Included here because it was found while testing this pin, by a sculptor using
the application rather than by any instrument either team built: every one of
those measures what happens *after* a segment fires, and this was a gate
deciding whether one fires at all. Mesh mode was correct throughout, and that
asymmetry was the whole tell.

## What this does not do

- **`clay_move_params.gesture_id`** stays unset. Our Move already replays from
  its anchor and its chain is one warp per dab per mirror image, so naming the
  gesture buys nothing until someone animates a radius or adopts `steady`. It is
  filed with the measurement attached rather than landed untestable.
- **Coarse-during-drag** is unblocked, not built. What it is worth now depends
  on what a drag costs after the re-mesh deletion, which needs a flat profile
  this workspace still cannot produce: `timed()` spans are nested and `FrameLog`
  keeps a max and a count but never a sum.
- **The export default stays at 0.5.** Reshaping a control to dodge an upstream
  defect would trade a visible warning for a silently weaker reduction, and the
  bands are a property of one mesh rather than of the ratio.
