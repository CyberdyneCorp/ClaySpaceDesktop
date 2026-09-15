# Prototype evidence

Twelve real mesh fixtures include sphere, box and 48-Grab meshes, full/subset requests and varied vertex attributes. Duplicate owner groups are inserted at every seventh brick; all benchmark variants compare complete remaining vertices and indices to the original algorithm.

Narrowing the vertex-table value to u32 gave inconsistent results between two optimized binaries: roughly 16.8 ms versus 9.0 ms for the same full-sphere candidate, reproduced over three alternating process pairs. It is not selected from this evidence. Keeping machine-sized vertex IDs and packing only triangle keys gave approximately 10.7→8.5 ms with in-place compaction in the expanded probe. Each probe repeats seven times in rotating variant order; warm-up is excluded. All exact-output comparisons pass. The quiet-start guard reports no sustained CPU contention.

This is temporary-probe evidence, not a production or application result. Wide-key fallback, hash collisions and packing boundaries need production regressions. Files are retained under `/tmp/clay-531-prune-{width,packed,repeat}*`.

## Implementation verification

The implementation keeps machine-sized vertex IDs, uses packed u128 triangle keys only when the input count proves 32-bit IDs, and retains the original wide-key fallback. Surviving complete triangles compact in place; the existing no-op when no complete triangle exists is preserved. Three new tests cover lane extremes and domain selection, forced-collision equality in both key paths, stable owner/winding/index behavior with trailing indices, and the no-complete-triangle case. All 47 strict OpenSpec items pass. Release compilation and correctness verification are complete as recorded below; application timing remains pending.

## Pinned-engine correctness and memory

All 90 enabled application cases pass against the committed Core v0.113.0 pin: 73 library and 17 native/rendered cases. One informational timing case is ignored, with no adapter skips. Format and all 47 strict OpenSpec items pass. Clippy with warnings denied passes. Cognitive-complexity threshold 12 reports no warnings in geometry.rs, including the new helpers and tests; warnings elsewhere are not claimed as resolved. Application timing remains pending.

A standalone allocation counter compiles the actual old and new pruning function bodies, using equivalent ten-word vertex storage and the twelve real fixtures with duplicate owners. All remaining vertices/indices match exactly. In the full-sphere fixture, allocation calls fall from 1,675 to 839; cumulative requested bytes from 31,380,848 to 23,271,400; and peak allocation above the pre-call live baseline from 25,984,712 to 21,777,112 bytes. The counter accounts for allocations, reallocations and frees; these are allocator payload measurements, not resident memory. Fixture construction/cloning is outside the measured interval. The wide-ID fallback retains its original key width. Source and CSV are `/tmp/clay-531-packed-production-memory.{rs,csv}`.

The combined application builds successfully with host `790374f` and Core `be189064` (same production engine as `264eed46`). Its preserved binary SHA-256 is `799ac8ccb22c5c739e8a7187ae6e80c73096c77548f55a63eaf6fe8d9cd10852`. The committed v0.113.0 vendor pin is restored; all 90 enabled combined application tests pass (73 library and 17 native/rendered), with one informational case ignored and no adapter skips. Combined application timing remains pending.

## Production pruning timing

The actual old and new pruning function bodies complete 168 timings across twelve real fixtures, seven repeats and alternating variant order. Excluding each warm-up, all twelve fixture medians improve; the full-sphere case is 10.300→8.948 ms and the corresponding attributed case is 9.673→9.270 ms. Every result matches the complete reference vertices and indices exactly. Fixture cloning is outside the timer, hash seeds vary by repeat, and the timing executable has no allocation-counter instrumentation. The quiet-start guard records no sustained CPU contention. This isolates pruning, not total brush latency; the all-brush application comparison remains pending. Sources, CSV and summary are retained under `/tmp/clay-531-packed-production-*`.

## Application comparison interrupted by contention

The first comparison completed seven alternating pairs (182 cases across all thirteen brushes), then was deliberately interrupted by the CPU guard during pair eight after 205 total cases. The 23 cases in the incomplete pair are excluded from paired analysis. This was sustained CPU contention, not an application command timeout. The full ten-pair comparison remains unfinished; no complete application latency benefit is claimed for this increment. Raw partial measurements and CPU history are retained under `/tmp/clay-531-packed-pruning-live*`. The remaining pairs must run in a quiet window.

## Completed application comparison

The remaining three alternating pairs complete in a quiet window without sustained contention or command timeouts. Combining those 78 cases with the first seven complete pairs gives 260 cases across all thirteen brushes; every paired begin/continue/end upload count matches. The incomplete 23-case pair remains excluded.

| Action | Previous median ms | Packed median ms |
|---|---:|---:|
| Standard release | 16.245 | 15.940 |
| Inflate release | 17.341 | 15.848 |
| Layer release | 16.983 | 15.885 |
| Clay release | 17.005 | 16.623 |
| Crease release | 22.371 | 19.989 |
| Move release | 47.096 | 45.002 |
| Relax preparation | 74.433 | 69.148 |
| Smooth preparation | 63.633 | 65.820 |
| Smooth release | 67.344 | 67.027 |
| Snake Hook release | 27.199 | 26.068 |

These are fixture medians, not per-run guarantees. Results are mixed, including slower Smooth preparation and a small Mask release increase. Nine brush release medians still exceed 16 ms. The goal remains open. Complete rows and summary are `/tmp/clay-531-packed-pruning-live-{complete,summary}.json`.
