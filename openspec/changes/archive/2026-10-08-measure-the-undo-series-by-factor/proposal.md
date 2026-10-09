# Measure the undo series by factor

## Why

Issue #174 measured an undo on a flat layer going from 20 ms to 4.5 s over twenty edits, and asked for a regression test that re-runs that series. The 200x was two factors multiplied together:

- **Region.** The undo of a Move grab refilled its node's whole bound, dilated by every earlier pull. ClayCore v0.120.1 reports the head links' balls instead (ClayCore #639 / #648). `ClayDocument::undo` already refills from `clay_document_undo_bound`, so the app picked the fix up with no code change.
- **Price of a brick.** Every refilled brick evaluates the whole deformer chain, so this price still grows with the edit count. It is the only factor a regional collapse (`bound-the-chain-by-region`) could still be credited with.

No test held either factor. A pin that brought the node bound back, or a change on our side that stopped refilling from the engine's bound, would bring back the 200x and no test would fail.

## What Changes

- Add `crates/clayspace-engine/tests/undo_series.rs`: twenty mirrored Move gestures on one patch, with the undo of each timed (fastest of five) and the bricks it re-meshed counted. The series is printed split into the two factors.
- Assert the region factor exactly: an undo re-meshes the grab's neighbourhood, which is under a quarter of the surface and never more than 1.5x the first undo's bricks.
- Assert the wall time loosely: the last three undos average under 8x the first three. That catches the 200x class on a shared runner without trying to referee a 2x.
- Time a 60-sample Snake Hook pull segment by segment, and assert that its late segments stay under 6x its early ones.
- Re-record the figures in `bound-the-chain-by-region` and `docs/why-move-was-slow.md`, and correct the tripwire's name in both.

## Measured (Mac, v0.120.1, release, four runs)

| | gesture 1 | gesture 20 | last three against first three |
|---|---:|---:|---:|
| undo | 0.30–0.32 ms | 0.81–0.94 ms | 2.00–2.31x |
| bricks re-meshed | 126 | 144 | 1.04x |
| price of a brick | 2.4–2.5 µs | 5.6–6.5 µs | 1.93–2.24x |

A 60-sample pull measured 0.41–0.43 ms per segment early and 0.96–1.07 ms late in release (2.27–2.50x), and 1.65–3.10x on a debug host. An undo over a baked patch still measures 4.4x an undo over the chain on this pin, so compaction stays off.

## Impact

Test and documentation only. No behavior changes. On this fixture, #174's 2x undo target is not met (2.0–2.3x). The remainder is the price of a brick, which only the engine can lower.
