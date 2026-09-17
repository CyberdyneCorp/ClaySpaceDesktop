# Batch whole-surface uploads

## Why
ClayCore #531 still exceeds the 16 ms brush-action target. Whole-surface layout in the desktop SDF path writes vertices and padded indices separately for each brick, causing thousands of queue writes during preview initialization and settlement.

## What Changes
Build contiguous staging arrays for fresh layouts and issue one vertex write and one index write. Preserve brick headroom, indexed vertex bits, index padding, bounds, and subsequent incremental patches. Measure CPU staging and total application latency before accepting the tradeoff of uploading unused vertex headroom.

## Impact
Private desktop surface geometry; no engine pin, brush math, file format, or public API changes. This is an experiment until paired timing and rendering verification pass. The full 16 ms goal remains open.
