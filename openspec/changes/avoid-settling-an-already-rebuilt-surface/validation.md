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


## Final platform and combined validation

All 16 applicable GitHub checks pass at production revision `e99ace5b`, including
Linux CPU/Vulkan, macOS CPU/Metal, formatting/lint, performance and cross-platform
document agreement. The baseline-recording job is intentionally skipped.

The same host revision also passes all 12 combined native MCP, sculpt-latency,
settlement and rendered-brush tests with Core `9cc0d181`, CPU fields and RTX 5060
Vulkan rendering, without adapter skips. The host's committed engine pin remains
v0.113.0 (`260b7797`). These results do not establish the wider issue's 16 ms target;
required settlement and expensive region operations remain above that budget.

## Follow-up: exact release compaction (in progress)

Attribution on host e99ace5b with the committed Core v0.113.0 pin compares complete vertex float bits, including normals, colors and masks. Six Standard dabs produce 283,682 stored triangles but exactly the same 283,152 distinct triangles as a full rebuild. Eight ordinary tools with 24 two-sample dabs each also match exactly. The existing long mixed-session geometry fixture stores 599,465 triangles, of which 588,184 are distinct; the full rebuild has exactly those 588,184 triangles. Its remaining 11,281 copies are exact duplicates. This does not change the known rendered long-session defect or count its ignored visual test as passing.

These probes justify investigating release compaction for synchronized document-gradient geometry. Preview/face shading is excluded. The new repeated-release regression fails with the previous full-rebuild route (Standard reports Bricks instead of Compact), then passes for all eight tools after implementation. The final pinned-engine build passes 17 cases: five settlement regressions, three native MCP end-to-end, four sculpt-latency, two rendered-brush and three rendered-incremental cases. Preview deferral, unsynchronized preview fallback and explicit rebuild semantics are covered. Strict OpenSpec validation passes all 45 items. After extracting mesh reading, world-coordinate conversion and mask sampling into a focused helper, all 17 cases pass again. Clippy with warnings treated as errors passes; no changed geometry function or new release/test helper exceeds cognitive complexity 12. The final combined-engine and live release comparisons are recorded below; new platform CI remains pending.

### Release compaction timing attempt

Three alternating before/fixed application runs covered all 13 brushes, 78 tool/run cases. Both binaries used Core `9cc0d181`; the baseline host was `e99ace5b`, and the fixed binary was the release-compaction prototype before storage cleanup. Each tool started after undoing previous edits, then measured begin at `[0,0,1]`, continue at `[0.12,0,1]`, and end, pressure 1. CPU fields and RTX 5060/Vulkan rendering were unchanged. No local builds/tests ran concurrently, but external C/C++ compilers raised one-minute machine load from 10.07 to 32.19. A fixed Move control release took 5.39 seconds in one run. These are contended observations, not an interactive-latency claim or validation of the final storage-cleanup code's speed.

| Tool | Baseline median end ms | Prototype median end ms |
|---|---:|---:|
| mask | 35.251 | 18.588 |
| crease | 448.520 | 148.435 |
| clay | 368.220 | 87.050 |
| inflate | 495.968 | 144.809 |
| layer | 536.460 | 199.581 |
| standard | 443.028 | 68.409 |
| polish | 367.696 | 233.497 |
| planar | 408.331 | 201.833 |
| move-topological | 658.715 | 232.855 |
| move | 387.187 | 318.410 |
| relax | 504.416 | 589.078 |
| smooth | 599.770 | 601.498 |
| snake-hook | 534.898 | 185.560 |

### Storage cleanup verification

A release previously relied on the full rebuild to discard empty entries and old vertex allocations. The new storage regression fails with three retained entries where one is expected. Release compaction now removes empty and duplicate-only entries, removes unreferenced vertices, remaps surviving indices without changing order or winding, and shrinks spare vertex/index capacity. One scratch remap buffer is reused across keys. Ordinary duplicate-pruning diagnostics keep their original vertex-array contract. Layout consumes prepared release geometry without repeating duplicate pruning.

The pinned-engine build passes 67 library tests (one informational timing test intentionally ignored) and 17 native/rendered integration tests. The final unit rerun additionally checks that oversized backing capacities are released. Clippy with warnings denied and all 45 strict OpenSpec items pass. Changed geometry and new release/test helpers remain within cognitive complexity 12. The final combined host/Core `9cc0d181` build also passes all 84 enabled cases (67 library and 17 native/rendered integration), with only the library timing probe intentionally ignored. The committed engine pin is restored to v0.113.0. New platform CI remains pending; the final live comparison follows.

### Final live release comparison

The final `b47c2633` implementation, including storage cleanup, was compared with host `e99ace5b`, both using Core `9cc0d181`, in three alternating runs across all 13 brushes (78 tool/run cases). The fixture, coordinates, renderer and measurement protocol were the same as the earlier attempt. One-minute load stayed between 2.125 and 2.276 on the 24-logical-CPU machine. A formatter used one CPU during part of the run; the earlier compiler saturation was absent. All cases completed.

| Tool | Baseline median end ms | Final median end ms |
|---|---:|---:|
| mask | 7.336 | 7.493 |
| crease | 76.301 | 25.783 |
| clay | 75.037 | 22.288 |
| inflate | 71.995 | 21.437 |
| layer | 74.727 | 25.192 |
| standard | 69.329 | 23.843 |
| polish | 90.724 | 39.061 |
| planar | 91.638 | 39.280 |
| move-topological | 107.876 | 53.029 |
| move | 67.225 | 67.603 |
| relax | 88.141 | 88.872 |
| smooth | 94.537 | 93.499 |
| snake-hook | 82.948 | 34.973 |

Standard release falls from 69.329 to 23.843 ms, Clay from 75.037 to 22.288 ms, and Snake Hook from 82.948 to 34.973 ms. Move and Smooth retain their necessary preview/epoch work and remain roughly unchanged. The final application's logs report `Compact` with zero engine mesh/read time; compaction itself still takes roughly 17–22 ms in logged cases. These results establish the release improvement, not a universal 16 ms outcome. Smooth/Relax pointer-down remains around 90–103 ms in this run.

## Release-compaction platform verification

All 16 applicable GitHub checks pass at production revision `b47c2633`, including Linux CPU/Vulkan, macOS CPU/Metal, formatting/lint, performance, packaging and cross-platform document agreement. The baseline-recording job is intentionally skipped. This verifies the final release-compaction/storage implementation; the later fast-hashing follow-up has its own pending CI.
