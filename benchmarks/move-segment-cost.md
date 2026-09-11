# What a live Move segment costs, and what makes it cost that

> **Read the last section first.** Everything above it measures cost *within
> one gesture*, which is not what was reported. The actual reproduction —
> "almost a second after 3 or 4 move dabs in a simple sculpture" — is about
> what accumulates *across* gestures, and it has a different cause, a
> different fix, and a much larger effect. The within-gesture findings stand;
> they were aimed at the wrong question.

Measured 2026-09-10 on this machine (macOS, aarch64), Release, against the
pinned engine — `vendor/ClayCore` at `662e132`, **0.84.0**. Instrument:
`benchmarks/probes/move_segment_cost.rs`.

Written to settle a report — *"the performance of the Move brushes in SDF is
not realtime"* — against three candidate explanations. **Two of the three are
dead and the third is the answer.**

## The two that died

**Warp accumulation is not it.** The live drag holds one grab per *gesture*,
not one per segment: `live_transactions.rs` asserts 12 against a segmented 72,
and that migration already shipped.

**The draw-then-undo pattern is not it.** ClayCore priced the two patterns
against each other in C — drawing each resolved grab and undoing it inside the
segment, versus refilling from `clay_sdf_move_preview_document` — over 240
samples per arm. The take-back arm costs **1.02x**, p95 1.00x. Two document
mutations and an undo round-trip are noise beside the refill they sit next to.
The preview-document entry point is the better design for other reasons — it
needs no undo, cannot leave a half-applied drag if a frame throws, and does not
grow the undo log — but *faster* is not one of them, and a lifetime-sensitive
hot-path refactor cannot be justified on 2%.

## The one that is the answer

**The scene is paid for on every pointer event.** Same drag, same radius, same
region, varying only how much form is under it:

| stamps in scene | mean ms / pointer event | bricks refilled per event |
|---|---:|---|
| 0 | 1.97 | 294 → 540 |
| 50 | 9.58 | 294 → 540 |
| 200 | 22.62 | 294 → 540 |
| 400 | 40.70 | 294 → 540 |

**The brick counts are identical down the column.** Every scene refills exactly
the same bricks over the same region, because the dirty region is the swept
ball of the drag and nothing about the scene changes it. What changes is what
one brick costs: about **0.005 ms** on an empty form and **0.10 ms** at 400
stamps, a factor of twenty.

At 200 stamps a pointer event costs 22.6 ms, which is 44 Hz. At 400 it is 40.7
ms, or 24 Hz. That is the report, reproduced.

## The other axis: radius, on an empty scene

| brush size | mean ms / pointer event |
|---|---:|
| 0.10 | 0.53 |
| 0.20 | 0.83 |
| 0.40 | 1.97 |

And within every block the rows climb as the drag lengthens — at radius 0.4,
294 bricks at event 4 and 540 at event 36. **That climb is the swept ball
growing, not waste.** A Move drag must refill where the surface was as well as
where it went, and both grow with the length of the drag; time tracks the brick
count (1.8x bricks, 1.7–2.0x time) rather than outrunning it, which is what a
quadratic would look like. This is not the Snake Hook shape that
`snakehook_segment_cost.rs` was written to catch.

## What this does and does not license

The lever is **per-brick evaluation cost in a scene with many items**, and
neither of the two dead candidates touches it. Two directions follow, in order:

1. **Engine-side, and the larger one.** Split the per-brick cost between
   compiling the request tape and evaluating it. If tape compilation is a per-
   refill cost that scales with item count, it is a strong candidate for being
   hoisted: during a live drag *only the dragged layer changes*, so the other
   399 stamps' contribution is recompiled per pointer event for a scene that did
   not move. ClayCore is measuring this split.
2. **Host-side, and smaller.** Fewer bricks in the region. The climb is
   legitimate, but `refill_preview` re-fills the union of the last preview box
   with this one, and for a drag that reverses toward its anchor that union is
   larger than the engine's own swept ball. Not isolated here — every drag
   measured was monotonically outward, where the previous box is nearly
   contained in the current one and the union costs approximately nothing.

## Caveats

One machine, one backend, one fixture shape — stamps scattered on a spiral over
the starting form at brush size 0.12. The **shape** transfers (region is a
function of drag geometry; per-brick cost is a function of scene complexity);
the absolute milliseconds do not. ClayCore's own probe measured 14.2 ms where
this one measures 22.6 at the same 200 stamps, on a different build, voxel size
and machine — same order, different number, and that gap is why neither figure
should be quoted as a device prediction.

---

# Follow-up: it is not the cull, it is what is under the brush

Added 2026-09-10, same machine and pin, after ClayCore measured the
compile/eval split. Their compile share came out **somewhere around 10–18%**,
with the trend across scene sizes inside their own noise — a first run read
16.6→17.4% and a repeat read 10.4→13.8%, on a box that was building
concurrently. Quoted as a range because that is what it is; the conclusion does
not depend on where in the range it lands, since at either end a tape-cache
hoist buys under a fifth and is not the answer. That measurement also reported that at 400 stamps the median brick
compiles a tape carrying ~449 of the document's ~481 instructions, and raised
the hypothesis that **the cull is barely culling**, with a blend pad that grows
with chain length as the suspect.

**That hypothesis does not hold on this host.** Three runs settle it.

## The cull drops what is far away

Same 400-stamp count, stamps confined to the hemisphere the drag is *not* on:

| stamps, far hemisphere only | mean ms / event |
|---|---:|
| 0 | 1.20 |
| 50 | 1.86 |
| 200 | 2.01 |
| 400 | 2.18 |

Against **40.70 ms** for the same 400 stamps spread over the whole form. Far
material costs 1.8x; the same material spread to include the drag site costs
20x. Whatever the tape length says, the region test is dropping what is far.

## How tight, exactly

400 stamps every row — only their distance from the drag changes. The drag is
radius 0.40 on a form of radius 0.95, so it subtends about 25 degrees:

| ring at N° from the drag | mean ms / event |
|---|---:|
| 0 | 385.28 |
| 30 | 211.58 |
| 60 | 15.16 |
| 90 | 6.33 |
| 150 | 1.86 |

**The fall between 30° and 60° is 14x.** The cull is tight, and it turns off
almost exactly where the drag stops reaching. The cost is the material actually
under the pointer.

## Distant items are not quite free if they are spread far apart

Separate balls added away from the drag, same count, varying only how large a
volume they occupy:

| balls | spread 0.5 | spread 4.0 | spread 16.0 |
|---|---:|---:|---:|
| 0 | 1.23 | 1.26 | 1.24 |
| 50 | 1.28 | 1.29 | 1.43 |
| 200 | 1.25 | 1.43 | 2.16 |
| 400 | 1.26 | 1.44 | 4.99 |

A tight cluster of 400 distant balls is free (1.26 against 1.23). The same 400
spread over a cube of side 16 cost 4x. Real but small beside the numbers above,
and noted rather than chased.

## What this means

**The report is local item density, not cull slack and not the drag pattern.**
A sculpt that stamps repeatedly in one area carries every one of those items in
that area, and every brick the drag dirties there evaluates all of them. At 400
stamps piled inside the drag region a pointer event costs 385 ms; the same 400
a hemisphere away cost 2.

Three consequences:

1. **Tightening the cull would win nothing here.** It is already turning off
   within 2x of the drag's own reach.
2. **ClayCore's fixture may be measuring this rather than a cull defect.** An
   arc of overlapping spheres in one chain is geometrically the 0° row above —
   everything is near everything, so a brick tape carrying nearly the whole
   document is the *correct* answer there, not evidence of a failure. Their own
   caveat anticipated this.
3. **The lever is collapsing local item count**, which is a different piece of
   work from anything discussed so far: baking a heavily stamped region back
   into one volume so the chain under the brush stops growing. The engine
   already has the verb — `Op::Replace` bake-and-replace, which Suavizar and
   Relaxar use. Nothing decides *when* to apply it during ordinary stamping.

   Note for anyone picking this up: the *whole-layer* version of this question
   is already answered in our pin. `advises_consolidation` comes off
   `field_report` (`document.rs:1663`) and the host already surfaces it.
   `clay_layer_consolidation_advice` is a later entry point and is **not** in
   0.84.0 — checked, not assumed. Neither answers the local case, which is
   what the ring table above is asking for.

Not proposing that here. Recording it because it is where the measurements
point, and because the two cheap fixes that were on the table — the preview
document and the tape cache — are now both measured and both small.

## The loose thread: smaller batches are not the lever

ClayCore explains the 4x on far-but-spread items structurally: `CullIndex` has
no spatial hierarchy, and `CullIndex::plan(region)` linear-scans it once per
**batch**, against the union of every brick box in that batch. A tight far
cluster misses that union and costs nothing; a spread far set touches it, so
every one of its items survives the plan and is walked once per brick before
the per-brick test correctly drops it.

That predicts the measurement. It also suggests an obvious lever — smaller
batches, tighter unions — so it was worth testing, because this host owns the
batch size (`drain_dirty`, `self.cache.take_dirty(512)`).

**It is not the lever.** Same fixture, 400 balls spread over a cube of side 16
and nowhere near the pointer, batch size varied 64-fold. The figure that
matters is the *delta* against each run's own 0-ball baseline, since the
baseline itself moves:

| batch | baseline | +400 balls | delta |
|---|---:|---:|---:|
| 512 | 2.09 | 7.94 | +5.85 |
| 128 | 1.40 | 5.67 | +4.26 |
| 32 | 2.11 | 6.27 | +4.17 |
| 8 | 3.72 | 8.47 | +4.75 |

The delta does not fall. If shrinking the batch shrank the union, a 64x cut
should have collapsed it; instead it sits between 4.2 and 5.9 ms throughout,
which is inside the run-to-run spread of this fixture. Small batches also cost
real money at the other end — the 0-ball baseline nearly doubles from 512 to 8,
which is the per-submission fixed cost the comment in `drain_dirty` already
warns about.

Two readings survived this measurement — either the batch union is not the
mechanism, or it is but `take_dirty` returns bricks in a non-spatial order so
the union never actually shrank. **ClayCore settled it: the first.** Recorded
here with their numbers because the conclusion is stronger than what this host
could reach alone.

The non-spatial escape hatch was never available. `BrickCache::mark_dirty`
fills its list with a z→y→x nested loop, `take_dirty` preserves that insertion
order, and the C paging call hands out a *contiguous slice* of that staged
vector — so a batch of 8 is 8 x-adjacent bricks, a thin strip rather than a
spanning sample. The union genuinely does shrink with the batch. It just does
not matter:

| batch | union volume | per-brick ms | plans |
|---|---:|---:|---:|
| 8 | 3.10 | 0.01521 | 81 |
| 32 | 7.63 | 0.01517 | 21 |
| 128 | 14.16 | 0.01446 | 6 |
| 512 | 28.88 | 0.01367 | 2 |

**The union collapses 9.3x and the per-brick cost does not move** — it gets
slightly worse at batch 8, which is the extra `plan()` calls. That is the flat
delta above, reproduced independently with the volume printed beside it.

So batch size is a dead end *and so is spatial grouping*: the coarse cull is
not what retains the far items, and sorting before `take_dirty` would buy
nothing. Both constants are closed from both sides.

The 4x itself stays **unexplained**. Ruled out: the cull pad, the batch union,
batch size, and `take_dirty` ordering. Not examined: `CullIndex` construction,
chain prunability, and what `plan()`'s linear scan does differently for a
clustered versus a spread entry set. It is small, it is filed, and it is worth
less than it would cost either side to chase right now.


---

# The report, finally reproduced: it is the chain across dabs

Added 2026-09-10 after the reproduction arrived: *"when I dab / stroke on
ClaySpaceDesktop it doesn't feel like ZBrush, Nomad3d, Blender, it takes almost
a second after I do 3 or 4 move dabs in a simple sculpture."*

**A simple sculpture is the low-density case.** Everything above says it should
be the fast one — 1.97 ms/event on a bare starting form. And the word is
*dabs*: three or four discrete gestures, where every measurement above was
taken inside a single gesture. So none of the earlier work was aimed at this.

Instrument: `benchmarks/probes/move_dab_cost.rs`. A plain starting form, eight
dabs, each a full open/segments/close, each landing somewhere new.

## What accumulates

| dab | chain | safe_step_scale | 256 raycasts | advises_consolidation |
|---|---:|---:|---:|---|
| 1 | 2 | 0.412423 | 0.56 ms | false |
| 2 | 4 | 0.170093 | 2.89 ms | false |
| 3 | 6 | 0.070150 | 3.73 ms | false |
| 4 | 8 | 0.028932 | 9.60 ms | false |
| 5 | 10 | 0.011932 | 22.33 ms | false |
| 6 | 12 | 0.004921 | 34.66 ms | false |
| 7 | 14 | 0.002030 | 39.30 ms | false |
| 8 | 16 | 0.000837 | 35.73 ms | false |

*(x mirror, which the starting form turns on by default.)*

**Every dab adds one grab per mirror image to the layer's deformer chain, and
the safe step scale is a product over that chain.** So it decays
geometrically: 0.41 to 0.0008 over eight dabs, a factor of 490. Sphere-tracing
the field then burns proportionally more iterations per ray, and 256 rays go
from 0.56 ms to 39 ms — **70x**. A viewport casts one ray per pixel, not 256.

That is the reported second, and it lands *after* the edit returns, which is
exactly how it was described.

## Three things this says

**1. The transaction fixed the wrong axis — and only half the problem.**
`live_move.rs` and `a_session_of_drags_steepens_by_the_drag_and_no_longer_by_
the_segment` establish that a drag costs one grab per *gesture* rather than one
per segment. True, and it was worth doing. But one per gesture still
accumulates, and nothing collapses it. The header of `live_move.rs` already
records the same decay measured per *drag* before the transaction existed —
"twelve drags took the step scale from 0.264 to below what a float reports".
That sentence was describing the problem that is still here.

**2. Symmetry doubles the rate, and it is on by default.** With no mirror the
chain grows by one per dab and reaches 0.0289 at dab 8; with the x mirror it
grows by two and reaches 0.0289 at dab **4**. The starting form turns x on, so
the default document degrades twice as fast as the measurements without it.

**3. The engine's advisory is silent on purpose, and the host is reading the
wrong field.** `advises_consolidation` is `false` at every row, including a
safe step scale of 0.000837 — and that is **correct**, not miscalibrated. It is
keyed on the *mechanism*: for a layer whose degradation is all deformer chain
there is nothing to absorb, and the engine has measured the bake at **6x
worse** — it swaps a cheap analytic item for a dense volume, and a 29x better
step scale is swamped by what the volume costs per sample. Had the flag fired
and the host acted, the sculptor would have got a slower layer.

The field that *does* describe this is `clay_field_report.degradation`, which
has been in the ABI since 0.70.0 and reads `CLAY_DEGRADATION_DEFORMERS` here.
The header says outright: **"READ `degradation` BEFORE ACTING."** This wrapper
was not carrying it, so a degraded layer and a healthy one produced the same
`false` and nothing else. Measured after binding it:

| dab | chain | step_scale | advises | degradation |
|---|---:|---:|---|---|
| 1 | 1 | 0.666644 | false | `None` |
| 2 | 2 | 0.444414 | false | `Deformers` |
| 8 | 8 | 0.296266 | false | `Deformers` |

The engine names the mechanism from the second dab onward.

## Where the fix is

Not in the drag path, not in local item density, and **not in consolidation**.
Baking is the cure for stacked volumes and a long edit list; it is measured 6x
worse for a chain of grabs on a layer with nothing to absorb, which is what a
session of dabs builds. An earlier draft of this note recommended exactly that
bake, on the strength of the advisory being silent. It was wrong.

What is left is genuinely open:

- **The crossover is unmeasured.** The engine's "6x worse" was taken on a real
  gesture, almost certainly a shallow chain. The table above reaches a 490x
  degradation. At that depth 6x more per sample against 490x fewer steps may
  well invert, and nothing in the current condition has an escape hatch for
  "degraded so badly that even a bad cure wins". ClayCore is taking this.
- **A collapse that keeps the layer parametric** — resolving N grabs into
  fewer without going to a volume — would be the right cure if one exists.
  `price-the-warps-a-layer-carries` declined both available forms as unsound,
  so it is research rather than a patch.

Host-side, one thing is now possible that was not: `degradation` distinguishes
a layer that wants nothing from one that is failing, so the application can at
least *tell*, and warn, rather than treating both as healthy. Whether it should
is a design question and is not decided here.

## Caveat

One machine, Release, CPU backend. The 256-ray figure is a stand-in for a
render, not a render. What transfers is the mechanism and the shape: chain
length grows per dab, step scale is a product over it, and march cost is
inverse to step scale. The absolute milliseconds are not a device prediction.
