# Validation

The screening probe replays twelve real mesh attribute/index fixtures through an equivalent engine-copy stand-in and the original/new readback routines. It completes 168 timings in a quiet CPU window with complete bit/index equality. All fixture medians improve; the first full-sphere fixture is 3.043→1.569 ms. This does not measure actual FFI readback or total brush latency.

The allocation probe extracts the current production readback body and original body, with the same allocation-free copy stand-in. Full output comparisons pass across all twelve fixtures. The first full-sphere fixture falls from three allocations / 14,639,200 cumulative and peak payload bytes to two allocations / 9,008,720 bytes. This isolates readback-owned allocations; it is not a measurement of engine-internal allocations or resident memory. Sources and rows are under `/tmp/clay-531-direct-readback/`.

Fourteen focused geometry cases pass against the committed engine pin, including new real-engine full-bit comparisons for colored/uncolored/empty meshes and preserved missing-normal errors. The existing informational timing case is ignored. The retained actual-engine readback timing test is intentionally ignored during correctness runs. All 93 enabled pinned-engine cases pass (76 library and 17 native/rendered), with two informational cases ignored and no adapter skips. Formatting, Clippy with warnings denied and all 49 strict OpenSpec items pass. A separate complexity check at threshold 12 reports no geometry.rs warnings. All 93 enabled combined cases also pass with host `659c10c` and Core `06b2306c`, with two informational cases ignored and no adapter skips. The preserved binary SHA-256 is `de0fc86221f94e1e5deca915bdd1c31a0bbc652736685b981221ba5cef2118de`; the committed engine pin is restored. Completed actual-engine and application timing follows below. Big-endian runtime behavior is not tested here.

## Actual-engine component timing

The retained `geometry::tests::profile_mesh_readback` test runs on the combined production binary in a quiet CPU window. It builds six real sphere cache meshes (voxel sizes 0.2, 0.1 and 0.025, with/without colors), compares the actual engine-copy paths over seven alternating repeats and verifies complete output outside the timer. All 84 timed outputs match. Excluding warm-up, all six fixture medians improve. At voxel size 0.025, uncolored readback is 2.296→1.494 ms and colored readback 2.455→1.578 ms. No sustained CPU contention is detected. Raw output and summary are `/tmp/clay-531-readback-production.log` and `/tmp/clay-531-readback-production-summary.json`.

## Completed application comparison

Ten alternating pairs complete 260 cases across thirteen brushes without command timeouts or sustained CPU contention. Every paired begin/continue/end upload count matches. Both binaries use the same production engine; this isolates the direct-readback host increment.

| Action | Previous median ms | Direct-readback median ms |
|---|---:|---:|
| Smooth begin | 57.816 | 55.745 |
| Smooth end | 59.188 | 58.720 |
| Relax begin | 63.105 | 63.231 |
| Relax end | 59.651 | 59.516 |
| Move end | 38.690 | 38.093 |
| Move Topological end | 50.888 | 55.437 |
| Snake Hook end | 23.933 | 23.439 |
| Standard end | 13.531 | 13.589 |
| Layer end | 13.344 | 13.800 |
| Inflate end | 13.949 | 14.332 |
| Clay end | 14.021 | 14.162 |

Results are mixed; smaller readback and allocation costs do not imply every brush action improved. Move Topological and Planar release are slower in this comparison, while most ordinary releases do not execute mesh readback and their differences are not attributed directly to this change. These are fixture medians, not per-run guarantees. Eight release medians and Smooth/Relax preparation still exceed 16 ms. Raw rows, empirical p90, maxima and over-budget counts are `/tmp/clay-531-readback-live/measurements.json` and `/tmp/clay-531-readback-live-summary.json`. Platform CI remains pending.
