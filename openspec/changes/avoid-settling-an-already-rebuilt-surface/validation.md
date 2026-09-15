# Validation in progress

Engine pin remains v0.113.0 (`260b7797`). Native tests use the RTX 5060 Vulkan renderer with CPU field evaluation.

## Regression

The fresh-surface mask release test was run against production sources at `2997c38` (which already include measured mask uploads). It fails because release uploads 10,982,672 bytes, taking 89.757 ms in that run. With the settlement provenance check, the same fresh-surface test passes with zero release uploads. The existing sequence after a field edit and undo retains old geometry from separate requests and still requires a nonzero release upload; the test explicitly preserves that case.

Three renderer tests pass: the existing six-dab triangle-union comparison, initial/partial/compaction/full/empty state transitions, and live Move preview commit. The committed-epoch surface is bit-identical to a separate full rebuild across complete vertex attributes. A further unit regression verifies that empty bookkeeping entries do not keep old triangle ownership alive.

## Broader checks

All eight selected targets pass: library tests (65 at the broad run, plus the subsequently added empty-entry regression), main unit tests (28), native MCP E2E (3), lod_switching (6), sculpt_latency (4), settle_needed (3), visual_brushes (2), visual_incremental (3). The informational pruning benchmark remains intentionally ignored in the normal suite and was run previously. No native adapter skip occurred.

New functions stay below the 12-point Clippy complexity threshold. Formatting, Clippy with warnings denied, and all 45 strict OpenSpec items pass.

## Pending

- Final PR documentation and platform CI.
- Remaining genuine brush work is not claimed to meet a universal 16 ms budget.

## Final live release comparison

Three alternating before/fixed runs across 13 tools (78 completed tool/run cases), using `2997c38` versus `72cddb6`, the same v0.113.0 engine, i9-12900K and RTX 5060 Vulkan renderer. Each run starts a fresh isolated native application; tools are isolated by undo to history depth zero. Default sphere, radius 0.18, strength 0.65, X symmetry, begin (0,0,1), continue (0.12,0,1), end. No other builds/tests from this task ran concurrently.

| Tool | End before ms | End fixed ms | Uploaded bytes before / fixed |
|---|---:|---:|---:|
| mask | 92.324 | 8.058 | 21,965,344 / 10,982,672 |
| crease | 78.023 | 84.623 | 11,200,824 / 11,200,824 |
| clay | 81.244 | 80.013 | 11,447,296 / 11,447,296 |
| inflate | 81.359 | 80.002 | 11,421,216 / 11,421,216 |
| layer | 86.154 | 80.617 | 11,199,800 / 11,199,800 |
| standard | 78.212 | 79.371 | 11,248,536 / 11,248,536 |
| polish | 102.204 | 111.370 | 12,240,448 / 12,240,448 |
| planar | 110.904 | 106.510 | 12,224,320 / 12,224,320 |
| move-topological | 152.862 | 155.095 | 12,843,392 / 12,843,392 |
| move | 161.910 | 86.118 | 21,965,344 / 10,982,672 |
| relax | 185.545 | 112.092 | 21,965,344 / 10,982,672 |
| smooth | 169.388 | 99.610 | 21,965,344 / 10,982,672 |
| snake-hook | 94.343 | 97.796 | 13,653,184 / 13,653,184 |

Mask, Move, Relax and Smooth halve their release upload counts. In this multi-sample mask stroke the final stroke sample still changes mask attributes, so release legitimately uploads those attributes once; the one-sample native regression proves zero uploads when attributes are already current. The eliminated work is the extra field rebuild. Standard's unchanged upload count and roughly unchanged latency are a control: partial-request settlement remains.

These measurements establish the release-work reduction for this configuration. Genuine region processing and required full rebuilds remain above 16 ms in several cases; no universal interactive budget is claimed.
