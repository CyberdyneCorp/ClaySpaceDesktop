# What a live Move segment costs, and what makes it cost that

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
