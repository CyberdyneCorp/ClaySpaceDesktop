# Validation record

Host base: `9a7ec9c`. Paired live runs use the same ClayCore `e04f1869`, CPU evaluator, NVIDIA RTX 5060 Vulkan renderer, default clean sphere, radius 0.18, strength 0.65 and X symmetry. Each tool starts after undo to history depth zero; samples are begin (0,0,1), continue (0.12,0,1), then end. One sample per action: diagnostic evidence, not a stable benchmark distribution.

The regression failed with the original forced-rebuild path: unchanged tool selection uploaded **10,982,672 bytes**, violating the zero-upload assertion (121.0 ms). With pending-work settlement it passed, including real rendered stroke changes, one gesture/undo, capture settlement, and a subsequent zero-upload wait. The MCP suite passed all 141 tests.

## Live measurements (milliseconds)

| Tool | Begin before / after | Continue before / after | End before / after |
|---|---:|---:|---:|
| mask | 108.3 / 0.3 | 105.4 / 0.0 | 107.2 / 133.1 |
| crease | 103.8 / 1.2 | 105.1 / 0.0 | 105.9 / 108.5 |
| clay | 110.0 / 4.0 | 110.0 / 0.0 | 112.5 / 110.5 |
| inflate | 107.4 / 2.5 | 105.1 / 0.0 | 107.8 / 109.8 |
| layer | 106.0 / 1.2 | 108.8 / 0.0 | 112.9 / 111.7 |
| standard | 103.8 / 1.2 | 100.6 / 0.0 | 111.4 / 110.6 |
| polish | 101.0 / 0.0 | 101.4 / 0.0 | 128.0 / 133.8 |
| planar | 107.9 / 0.0 | 110.4 / 0.1 | 141.6 / 156.7 |
| move-topological | 105.7 / 0.0 | 115.3 / 0.0 | 170.0 / 161.4 |
| move | 112.1 / 0.0 | 111.7 / 3.8 | 226.9 / 233.9 |
| relax | 414.3 / 292.9 | 107.1 / 0.0 | 271.2 / 259.1 |
| smooth | 402.4 / 299.8 | 106.3 / 0.0 | 275.8 / 251.0 |
| snake-hook | 107.5 / 0.0 | 129.4 / 18.3 | 127.4 / 124.1 |

The artificial common floor disappears from begin/continue. End still pays the existing full settlement, and Smooth/Relax still have expensive region work at begin. This change does **not** establish a universal 16 ms budget or eliminate final remeshing. Zero-upload continued samples only describe these particular brush samples; they are not a claim about all drags.

## Additional completed checks

- Both native E2E tests pass with the measured-edit upload assertions, against both `e04f1869` and the unchanged host pin `260b7797` (v0.113.0); no skips.
- Six settlement/gesture unit tests pass against the pinned engine.
- Workspace release Clippy with warnings denied, layering, formatting and strict change validation pass.
- Changed production functions do not exceed Clippy's cognitive-complexity threshold of 12. Rust is not supported by the skill's normal analyzer, so Clippy supplies this check.

The existing large E2E scenario measures 27 on the main version (`9a7ec9c`) and 27 after the change. It remains a flagged test-maintenance exception: one native application/session drives its sequential rendering/history checks. New named regression helpers stay below 12 instead of adding to that scenario's complexity. All 43 OpenSpec items pass strict validation.

## Pending

- PR CI.

## Mask completion correction

The early Mask begin timings above did not include its GPU attribute refresh: mask edits dirty no field bricks, and that refresh ran only during redraw. A new live regression fails on `41237d8`: measured Mask begin returns 0 uploaded bytes (0.291 ms). Completion now invokes the existing revision-guarded mask refresh, and pending mask revisions are reported by wait. Renderer initialization completes its initial refresh so an idle command does not inherit startup work.

Both native E2E tests pass after the fix, including positive uploads during measured mask painting, zero uploads on a following idle wait, idle selection, real field edits, rendered changes and undo. This corrects attribution; the old 0.3 ms Mask figure is not its complete render-update latency. New helpers remain below the 12-point Clippy complexity threshold.


## Final platform and combined validation

All 16 applicable GitHub checks pass at production revision `e99ace5b`, including
Linux CPU/Vulkan, macOS CPU/Metal, formatting/lint, performance and cross-platform
document agreement. The baseline-recording job is intentionally skipped.

The same host revision also passes all 12 combined native MCP, sculpt-latency,
settlement and rendered-brush tests with Core `9cc0d181`, CPU fields and RTX 5060
Vulkan rendering, without adapter skips. The host's committed engine pin remains
v0.113.0 (`260b7797`). These results do not establish the wider issue's 16 ms target;
required settlement and expensive region operations remain above that budget.
