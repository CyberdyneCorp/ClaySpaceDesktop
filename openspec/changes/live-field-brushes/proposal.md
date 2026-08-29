# Show a smoothing stroke while it is being made

## Why

Suavizar and Relaxar arrive only when the pointer comes up. The sculptor
smooths blind, which is what was reported: *"I only see the effect when the
stroke finishes."*

That was not an oversight. On a field these tools **bake** — the engine's relax
takes a sampled volume and returns another — so a stroke split into segments
adds a `Op::Replace` volume per segment and the surface crumbles.
`holds_the_whole_gesture` says so, and holding the gesture was the only
affordable shape available: baking the layer again per dab costs the
consolidation pass, 313 ms on ClayCore's reference iPad and 186 ms here.

ClayCore 0.60.0 removes the reason. `clay_sdf_smooth_begin/update/commit`
samples the layer **once**, relaxes its own retained volume per dab, and
installs that volume at commit. Between begin and commit the document does not
change at all.

## Drawing what has not been written down

The transaction hands over *samples*, and the viewport draws triangles the
engine meshed from a brick cache. Getting from one to the other without a
second implementation of the field is the whole design problem, and
`docs/architecture.md` is corrected as part of this change because it stated
the constraint too broadly: it said the viewport renders engine-produced meshes
because the kernel dialect does not target WGSL. That is true of *compiling
ClayCore's kernels into our own shader* and not of the other route ClayCore
documents — uploading what the engine already evaluated — which this repository
itself proposed as ClayCore#43 and which has been complete since ABI 0.25.0.
The rule is about whose arithmetic decides where the surface is, not about
which pixels may be drawn.

**The preview is relabelled, not resampled.** Its lattice has the spacing we
ask for and an origin of its own — the layer's bounds, less the padding —
which does not land on the brick cache's lattice and cannot be made to: one
`padding` cannot align three axes whose bounds have different remainders.
Measured, an off-lattice form puts the preview's origin at −13.12, −14.08 and
−13.38 voxels. So the preview keeps a cache of its own and preview brick `K` is
stored as that cache's brick `K`: the two lattices are then the same lattice in
a world translated by the preview's origin, and the translation is undone on
the vertices the engine meshes. Nothing is interpolated. The engine's mesher
sees the samples the transaction computed.

Three alternatives were measured and rejected:

| shape | per frame | why not |
|---|---:|---|
| snapshot into a scratch document | **~30 ms** | a fresh preview item re-pays the document's preparation every frame; ClayCore's own header calls this "the snapshot path, and not the per-frame one" |
| resample the delta onto the cache's lattice | ~1 ms | puts our arithmetic between the engine and the surface |
| **relabel into a cache of the preview's own lattice** | **~3–5 ms** | this |

## What it costs, measured on the starting form at the application's own 0.02

| | |
|---|---:|
| opening the gesture (sampling the layer, priming the preview) | 186 ms |
| one live dab | ~5 ms |
| laying the previewed surface out once, when it opens | 34 ms |

The 186 ms is a pointer-down cost and it is the trade the design makes: the
whole finite layer once, so that every dab afterwards costs what it touches.

## What this does not do

- **A second visible field subtool falls back to the held gesture.** The brick
  cache holds the hard union of every visible SDF layer and attributes no brick
  to the layer it came from, while a transaction previews one layer alone.
  Composing them would cost more per frame than the preview saves. The fallback
  is exactly the previous behaviour, so nothing regresses. Filed as
  [ClayCore#378](https://github.com/CyberdyneCorp/ClayCore/issues/378), with
  the three routes that would close it.
- **Move keeps the path it has.** `clay_sdf_move_*` is wrapped and tested, and
  measured it collapses a ten-segment drag's deformer chain from **10 to 1**.
  It is not adopted for the interactive drag, because the transaction writes
  nothing to the document until it commits and — unlike Smooth — it offers no
  samples a host can draw, only grab parameters to reproduce. Adopting it would
  take the live picture away from a tool that has one, which is the regression
  this whole change exists to undo.
