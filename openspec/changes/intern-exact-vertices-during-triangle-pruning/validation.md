# Validation in progress

## Native attribution and prototype

Host `7090817`, engine pinned v0.113.0 `260b7797`, Release, CPU field evaluation, NVIDIA RTX 5060 Vulkan rendering. Default clean sphere, radius 0.18, strength 0.65, X symmetry, Standard begin (0,0,1), continue (0.12,0,1), end, undo between strokes.

Temporary diagnostic timers split layout pruning from placement/upload. Original pruning measured 67.288/53.629/54.923 ms; placement/upload 2.463/2.450/2.482 ms. Reserving the original table reduced pruning to 39.526/39.039/38.346 ms. Interning complete vertices into u32 IDs reduced it to 13.614/13.819/10.946 ms. End-to-end release was 134.8/115.4/116.2 ms originally and 81.1/76.6/80.4 ms for that prototype. These sequential live samples identify the cost, not a portable latency gate. Temporary profiling code has been removed.

Production uses usize IDs to avoid narrowing a large stored surface. Its exact-output regression compares all per-key vertex bits and surviving indices against the original algorithm for all ten vertex components, signed zeros, distinct NaN payloads, duplicate corner orders and empty geometry. All 65 library tests pass (one informational benchmark ignored by default).

An extracted production-code workload probe with seven alternating pairs measured shared grid triangles 17.621 → 3.936 ms and unshared triangle soup 15.409 → 12.719 ms, after verifying exact output. The equivalent reproducible probe is now the ignored `profile_exact_triangle_pruning` library test. The intern table can cost more temporary memory for triangle soup than the original triangle-only set; the design does not claim universal memory savings.

## Rendered tests

Four suites pass their enabled tests on the real adapter: visual_holes (1), visual_incremental (3), lod_switching (6), sculpt_latency (4). No adapter skip occurred. The ignored long-session visual_holes case was explicitly run as well: its exact triangle-set pruning assertion passed, but its final image check failed with 5 pinholes and 7 dark specks. The same explicitly enabled test on a separate main-baseline checkout `9a7ec9c` fails with exactly 5 pinholes and 7 dark specks. Its long-session PNG is byte-identical to the optimized run (SHA-256 `af9be3ddcef5b6244b8710d67e51e97a5aa4f5ce175b00ba16f163cf21420e24`). The defect therefore predates this change. Both runs retain 588,184 distinct triangles after pruning 11,281 duplicates.

Clippy with warnings denied passes. Final changed production, reference-test and workload functions do not trigger the 12-point cognitive-complexity threshold.

## Remaining

- Verify platform CI. Full release remeshing and expensive region brush work remain; no universal 16 ms claim.

The committed informational workload test passed: shared geometry 18.536 → 4.007 ms, triangle soup 19.118 → 16.761 ms (seven alternating pairs per case, median). Both native MCP E2E tests pass against final production commit `0d14b44` with the unchanged v0.113.0 engine pin.

## Final production live comparison

Three alternating runs of main `9a7ec9c` and fixed `0d14b44`, both with the unchanged v0.113.0 engine, on an Intel i9-12900K / 24 logical CPUs and RTX 5060. Each run opens an isolated native application, restores history depth zero before each tool, and uses the same samples/configuration above. All 78 tool/run combinations complete successfully. No builds or other tests from this task ran concurrently, but unrelated C++ compilation heavily contended the machine. Absolute release timings fluctuate widely; this run does **not** establish a whole-application release speedup. The adjacent-pair workload probe and attributed prototype isolate pruning's improvement more reliably.

Median milliseconds across the three runs:

| Tool | Begin main / fixed | Continue main / fixed | End main / fixed |
|---|---:|---:|---:|
| mask | 589.65 / 0.58 | 496.49 / 0.07 | 484.11 / 453.42 |
| crease | 554.74 / 16.05 | 611.19 / 0.08 | 572.24 / 400.94 |
| clay | 645.59 / 23.40 | 531.16 / 0.09 | 521.85 / 572.82 |
| inflate | 650.03 / 15.01 | 600.85 / 0.09 | 695.20 / 531.84 |
| layer | 492.77 / 9.05 | 334.52 / 0.09 | 326.90 / 506.20 |
| standard | 633.31 / 12.58 | 459.39 / 0.09 | 584.98 / 458.27 |
| polish | 421.65 / 0.07 | 471.18 / 0.07 | 655.58 / 553.04 |
| planar | 464.47 / 0.09 | 460.14 / 0.08 | 720.01 / 442.99 |
| move-topological | 354.14 / 0.06 | 309.41 / 0.06 | 534.87 / 626.23 |
| move | 251.39 / 0.08 | 199.07 / 21.83 | 383.91 / 836.66 |
| relax | 570.81 / 657.68 | 164.09 / 0.08 | 1160.21 / 359.90 |
| smooth | 1173.88 / 351.39 | 257.00 / 0.06 | 1389.73 / 250.85 |
| snake-hook | 523.24 / 0.09 | 492.13 / 99.69 | 520.86 / 660.63 |

The common measurement floor is removed from ordinary begin/continue operations, while genuine expensive edits and release work remain. Some end medians worsen in this contended run; they are retained here rather than treated as a portable regression or discarded. Source inspection also confirms that epoch-changing tools can rebuild in command synchronization and then pay the existing unconditional release settlement again. That is a remaining host lifecycle cost, not removed by vertex interning.

The Mask begin figures in this historical comparison also exclude the deferred mask attribute refresh at those revisions. The subsequent mask-completion correction in the measurement change adds that work to the timed operation; see its validation record. Do not interpret the historical Mask timings as complete rendering costs.


## Final platform and combined validation

All 16 applicable GitHub checks pass at production revision `e99ace5b`, including
Linux CPU/Vulkan, macOS CPU/Metal, formatting/lint, performance and cross-platform
document agreement. The baseline-recording job is intentionally skipped.

The same host revision also passes all 12 combined native MCP, sculpt-latency,
settlement and rendered-brush tests with Core `9cc0d181`, CPU fields and RTX 5060
Vulkan rendering, without adapter skips. The host's committed engine pin remains
v0.113.0 (`260b7797`). These results do not establish the wider issue's 16 ms target;
required settlement and expensive region operations remain above that budget.

## Randomized scratch-table hashing (in progress)

An isolated release-mode comparison extracts the same pruning algorithm and existing workloads, replacing only the two temporary table hashers. Fifteen alternating pairs per workload compare complete vertex bits and surviving indices after every run (60 total runs). On this Linux i9-12900K host, median shared-grid pruning is 4.195 ms with the standard hasher and 3.014 ms with ahash; triangle soup is 15.769 versus 10.938 ms. These are isolated algorithm timings, not live release latency.

The production follow-up has an explicit ahash 0.8.12 dependency (already present in the lockfile), randomized state per pruning call, and full key equality. A regression forces all hashes to zero and separately tests four seeds against the original full-vertex reference across attributes, signed zero, NaN payloads, reversed duplicates and sorted owners. All 68 library correctness tests pass on the committed Core v0.113.0 pin; one informational timing test is intentionally ignored. Native integration, live timing and platform checks remain pending for this follow-up.

Formatting, Clippy with warnings denied and all 45 strict OpenSpec items pass for the hashing follow-up. An explicit Clippy cognitive-complexity run at threshold 12 reports no changed geometry or new test helper above that threshold. The broader application still reports other functions above 12; this is not a claim that every function meets the target.

All 17 native integration cases also pass on the pinned engine: three command-interface, four sculpt-latency, five settlement, two rendered-brush and three rendered-incremental tests. No adapter skips occurred. This establishes 85 enabled local cases for the follow-up; the isolated timing test remains ignored in the ordinary library run. Combined-engine validation, uncontended live timing and platform CI remain pending.

## Final hashing live comparison

Host `4e8d1af` passes all 85 enabled combined cases with Core `9cc0d181` (68 library, 17 native integration), with one intentionally ignored informational library timing test and no adapter skips. The committed engine pin remains v0.113.0.

Three alternating runs compare host `b47c2633` with `4e8d1af`, both using Core `9cc0d181`, CPU fields and the RTX 5060 Vulkan renderer. All 78 tool/run cases complete. Each tool starts from the same fresh sphere by undoing to history depth zero, then measures begin at (0,0,1), continue at (0.12,0,1), and end with pressure 1. No builds or tests from this task run concurrently. One-minute load stays between 4.120 and 4.419 on 24 logical CPUs. Uploaded byte counts match between versions for every action in every paired case.

| Tool | Before median release ms | Fast-hash median release ms |
|---|---:|---:|
| mask | 7.182 | 7.318 |
| crease | 25.567 | 22.196 |
| clay | 25.008 | 15.751 |
| inflate | 25.744 | 20.635 |
| layer | 26.197 | 21.018 |
| standard | 21.654 | 19.818 |
| polish | 49.223 | 34.663 |
| planar | 47.338 | 32.666 |
| move-topological | 59.661 | 51.129 |
| move | 74.241 | 68.250 |
| relax | 83.527 | 82.199 |
| smooth | 85.543 | 85.976 |
| snake-hook | 34.963 | 29.702 |

Standard and Clay release improve in each paired run, while individual timings still vary. Snake Hook has a 48.377 ms fixed-side outlier versus 36.878 ms in that paired baseline run; its median improvement is not a per-run guarantee. Smooth release is essentially unchanged by the median. Its fixed pointer-down median is 83.894 ms, Relax is 89.897 ms, and Snake Hook continue is 17.071 ms. The hash change reduces compaction cost but does not establish a universal 16 ms budget. Current platform CI for the hashing follow-up remains pending.
