# Batch SDF compaction uploads directly

## Why
ClayCore #531 still exceeds the 16 ms brush-action target. Whole-surface layout in the desktop SDF path writes vertices and padded indices separately for every brick. The first batching prototype added full CPU arrays and did not establish a latency win.

## What Changes
After compaction of retained geometry, prepare borrowed brick spans, then fill wgpu's mapped upload buffers directly with one vertex write and one index write. Preserve indexed vertex bits, index padding, live bounds, brick headroom, and subsequent incremental patches. Avoid allocating or copying whole-surface CPU arrays. Keep original per-brick uploads for full rebuilds and preview initialization: the completed application comparison found that batching those paths regressed latency.

## Impact
Private desktop geometry and renderer upload helpers; no engine pin, brush math or file format change. Add byte-reference regressions, a real GPU rendering test, and an informational comparison of the original, CPU-array and direct upload paths. Extra vertex-gap upload bytes remain a tradeoff. Validate whole-application latency before marking this ready; the full 16 ms goal remains open.
