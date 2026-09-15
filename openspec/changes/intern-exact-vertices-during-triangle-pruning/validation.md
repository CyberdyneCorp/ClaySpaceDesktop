# Validation in progress

## Native attribution and prototype

Host `7090817`, engine pinned v0.113.0 `260b7797`, Release, CPU field evaluation, NVIDIA RTX 5060 Vulkan rendering. Default clean sphere, radius 0.18, strength 0.65, X symmetry, Standard begin (0,0,1), continue (0.12,0,1), end, undo between strokes.

Temporary diagnostic timers split layout pruning from placement/upload. Original pruning measured 67.288/53.629/54.923 ms; placement/upload 2.463/2.450/2.482 ms. Reserving the original table reduced pruning to 39.526/39.039/38.346 ms. Interning complete vertices into u32 IDs reduced it to 13.614/13.819/10.946 ms. End-to-end release was 134.8/115.4/116.2 ms originally and 81.1/76.6/80.4 ms for that prototype. These sequential live samples identify the cost, not a portable latency gate. Temporary profiling code has been removed.

Production uses usize IDs to avoid narrowing a large stored surface. Its exact-output regression compares all per-key vertex bits and surviving indices against the original algorithm for all ten vertex components, signed zeros, distinct NaN payloads, duplicate corner orders and empty geometry. All 65 library tests pass (one informational benchmark ignored by default).

An extracted production-code workload probe with seven alternating pairs measured shared grid triangles 17.621 → 3.936 ms and unshared triangle soup 15.409 → 12.719 ms, after verifying exact output. The equivalent reproducible probe is now the ignored `profile_exact_triangle_pruning` library test. The intern table can cost more temporary memory for triangle soup than the original triangle-only set; the design does not claim universal memory savings.

## Rendered tests

Four suites pass their enabled tests on the real adapter: visual_holes (1), visual_incremental (3), lod_switching (6), sculpt_latency (4). No adapter skip occurred. The ignored long-session visual_holes case was explicitly run as well: its exact triangle-set pruning assertion passed, but its final image check failed with 5 pinholes and 7 dark specks. A separate checkout of main baseline `9a7ec9c` is being built to establish attribution; that comparison remains pending.

Clippy with warnings denied passes. Final changed production, reference-test and workload functions do not trigger the 12-point cognitive-complexity threshold.

## Remaining

- Run committed workload probe and final production live measurements without our concurrent build workload.
- Complete main-baseline long-session comparison.
- Check final complexity/format/OpenSpec and update PR title/body for the complete host change.
- Verify platform CI. Full release remeshing and expensive region brush work remain; no universal 16 ms claim.
