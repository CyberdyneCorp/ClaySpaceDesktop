# Prototype evidence

Twelve real mesh fixtures include sphere, box and 48-Grab meshes, full/subset requests and varied vertex attributes. Duplicate owner groups are inserted at every seventh brick; all benchmark variants compare complete remaining vertices and indices to the original algorithm.

Narrowing the vertex-table value to u32 gave inconsistent results between two optimized binaries: roughly 16.8 ms versus 9.0 ms for the same full-sphere candidate, reproduced over three alternating process pairs. It is not selected from this evidence. Keeping machine-sized vertex IDs and packing only triangle keys gave approximately 10.7→8.5 ms with in-place compaction in the expanded probe. Each probe repeats seven times in rotating variant order; warm-up is excluded. All exact-output comparisons pass. The quiet-start guard reports no sustained CPU contention.

This is temporary-probe evidence, not a production or application result. Wide-key fallback, hash collisions and packing boundaries need production regressions. Files are retained under `/tmp/clay-531-prune-{width,packed,repeat}*`.

## Implementation verification in progress

The implementation keeps machine-sized vertex IDs, uses packed u128 triangle keys only when the input count proves 32-bit IDs, and retains the original wide-key fallback. Surviving complete triangles compact in place; the existing no-op when no complete triangle exists is preserved. Three new tests cover lane extremes and domain selection, forced-collision equality in both key paths, stable owner/winding/index behavior with trailing indices, and the no-complete-triangle case. All 47 strict OpenSpec items pass. Release compilation and production verification remain pending.

## Pinned-engine correctness and memory

All 90 enabled application cases pass against the committed Core v0.113.0 pin: 73 library and 17 native/rendered cases. One informational timing case is ignored, with no adapter skips. Format and all 47 strict OpenSpec items pass. Clippy with warnings denied passes. Cognitive-complexity threshold 12 reports no warnings in geometry.rs, including the new helpers and tests; warnings elsewhere are not claimed as resolved. Application timing remains pending.

A standalone allocation counter compiles the actual old and new pruning function bodies, using equivalent ten-word vertex storage and the twelve real fixtures with duplicate owners. All remaining vertices/indices match exactly. In the full-sphere fixture, allocation calls fall from 1,675 to 839; cumulative requested bytes from 31,380,848 to 23,271,400; and peak allocation above the pre-call live baseline from 25,984,712 to 21,777,112 bytes. The counter accounts for allocations, reallocations and frees; these are allocator payload measurements, not resident memory. Fixture construction/cloning is outside the measured interval. The wide-ID fallback retains its original key width. Source and CSV are `/tmp/clay-531-packed-production-memory.{rs,csv}`.
