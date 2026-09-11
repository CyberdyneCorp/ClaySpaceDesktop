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
