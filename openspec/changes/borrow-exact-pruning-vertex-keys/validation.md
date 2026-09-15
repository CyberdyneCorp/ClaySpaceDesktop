# Validation

The screening probe completes 168 timings across twelve real full/subset sphere, box and 48-Grab fixtures, with duplicate brick owners and varying hash seeds. All complete retained vertex/index values match the current packed-key reference. Excluding warm-up, full-sphere pruning improves 8.258→6.869 ms; every fixture median improves. The quiet CPU guard reports no sustained contention. This is prototype component evidence, not total brush latency.

The production allocation probe extracts both actual function bodies and uses equivalent ten-word vertex storage. All twelve full output comparisons pass. Full-sphere allocation calls remain 839; cumulative requested bytes decrease 23,271,400→14,887,560 and peak allocation above the pre-call baseline decreases 21,777,112→13,393,272 bytes. Fixture creation/cloning is outside the measured interval. These are allocator payload measurements, not resident memory. No instrumented timing is used. Sources and results are `/tmp/clay-531-borrowed-pruning/{build-probes.py,memory.rs,memory.csv}`.

The pinned-engine focused tests pass twelve cases, with one informational timing case ignored. They include full-bit reference equality under forced hash collisions, special float bits in all ten channels, both packed and wide paths, stable ownership/order and the new repeated release/storage-compaction regression. All 91 enabled pinned-engine cases pass (74 library, 17 native/rendered), with one informational timing case ignored and no adapter skips. Format and all 48 strict OpenSpec items pass. Clippy with warnings denied passes. The separate cognitive-complexity check at threshold 12 reports no geometry.rs warnings. The combined application (host `a008779`, Core `06b2306c`, same production engine as `ebf084ab`) also passes all 91 enabled cases, with one informational case ignored and no adapter skips. Its preserved binary SHA-256 is `3fa2830f79485b318f950bd6fa69c9794e7d3805ab120e120d2d710f134cc26d`. The committed engine pin is restored. Completed production and application results follow below.

## Production component timing

The actual before/current function bodies complete 168 timings over twelve real fixtures, seven repeats with alternating variant order and varied hash seeds. All complete output comparisons pass. Excluding warm-up, all twelve medians improve: full-sphere pruning 9.215→7.509 ms and the attributed fixture 8.551→7.040 ms. The quiet CPU guard records no sustained contention. Timing is uninstrumented and fixture cloning occurs outside the timer. Source, rows and summary are `/tmp/clay-531-borrowed-pruning/production{.rs,-timing.csv,-summary.json}`.

## Completed application comparison

Ten alternating pairs complete 260 cases across thirteen brushes without command timeouts or sustained CPU contention. Every paired begin/continue/end upload count matches. Both binaries use the exclusive-edge engine implementation; this comparison isolates the host pruning change.

| Action | Before median ms | Borrowed median ms |
|---|---:|---:|
| Standard end | 15.347 | 13.691 |
| Layer end | 15.432 | 13.901 |
| Inflate end | 15.697 | 14.183 |
| Clay end | 16.027 | 14.525 |
| Crease end | 20.920 | 17.564 |
| Snake Hook end | 26.091 | 24.263 |
| Smooth begin | 58.018 | 57.080 |
| Smooth end | 62.757 | 61.405 |
| Relax begin | 64.485 | 64.098 |
| Relax end | 62.291 | 61.177 |
| Move end | 40.260 | 39.553 |
| Move Topological end | 48.048 | 51.568 |

These are fixture medians, not per-run guarantees. Results are mixed, including slower Move Topological release. Eight release medians and Smooth/Relax preparation still exceed 16 ms. Raw rows, empirical p90, maxima and over-budget counts are `/tmp/clay-531-borrowed-live/measurements.json` and `/tmp/clay-531-borrowed-live-summary.json`. The all-brush goal remains open; platform CI is pending for this increment.
