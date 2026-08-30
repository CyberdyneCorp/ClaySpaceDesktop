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

ClayCore 0.60.0 removes the reason. `clay_sdf_smooth_begin/update` samples the
layer **once** and relaxes its own retained volume per dab, touching nothing in
the document. That is what the preview is made of.

**Its commit is not taken.** `clay_sdf_smooth_commit` installs the working
volume as the layer's one item — it consolidates the whole subtool, on every
stroke. On this machine that measures slightly better than the bake it would
replace (roughness 5.74 against 5.83 on the reference roughened surface); on
the Metal runner it measures **7.82 against a ceiling of 6.00**, moving 2458
pixels where the same stroke moves 205 here. Planar and Polir, baked the old
way, are identical across both platforms, so it is the consolidation and not
the measurement. Filed as
[ClayCore#379](https://github.com/CyberdyneCorp/ClayCore/issues/379).

Even where it measures well it is a heavy thing to do on every stroke: it
discards the layer's edit list and re-samples the whole subtool at the cache's
cell size, so repeated smoothing compounds the resampling. So the transaction
draws the preview and the stroke is laid down by the bake that was always used
— which reproduces the old numbers **exactly**, 5.83 and 188 pixels, on every
backend.

The cost is that the preview and the result are not the same arithmetic: the
preview relaxes cumulatively per dab, the bake makes one pass over the whole
gesture. Measured, they land 0.09 apart in roughness and under a hundredth of a
unit apart in where the surface stands on a form of radius one. The spec asks
for agreement within a stated tolerance rather than identity, because identity
is not what is delivered.

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

## Why no test caught it, and what now does

Two gaps, and each hid the other. Only two tests ever opened a gesture, and
both stroked with a *dragging* verb — `visual_snakehook` with Puxar,
`mesh_move` with Mover — which is precisely the case where taking the last
segment back is right. And the tests that drag a *stamping* brush go through
`SharedDocument`, which never forwarded `begin_gesture`, so the branch was
unreachable from the only place that would have exercised it.

`shared_forwarding.rs` exists for exactly this class of fault. It could not
catch this one because it is a hand-written list, which is the same shape as
the mistake it guards against. It now also reads the traits, finds every method
with a body, and requires the shared document to name it — and that found
**four more**, all implemented on the document and all answered by the trait's
inert default in the running application:

| | what it meant |
|---|---|
| `set_alpha`, `alpha_name` | a loaded alpha stamp was swallowed, and the options bar reported no stamp in use |
| `apply_sculpt_layer_op`, `sculpt_layer_cost` | recording passes on a voxel layer always refused, and its cost always read as nothing |

All four are forwarded here. Three provided methods are *derived* — their
default is written in terms of other trait methods and reaches the document
through those — and are listed in the check with the reason.

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
