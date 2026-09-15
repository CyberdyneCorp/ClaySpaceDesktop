# Prototype validation

A standalone Rust prototype compares the original per-brick hash map, the same algorithm with the already-used randomized fast hasher, and a dense remap. Twelve real engine mesh/range fixtures cover sphere, box and a 48-Grab sphere, full and subset requests, with/without gradient/color attributes. Synthetic mask payloads include negative zero and NaN bits.

All three approaches agree on complete vertex bits and local indices for 10,340 processed brick layouts. The second pass reverses brick processing order; empty and invalid-index groups exercise clearing and the existing default-placeholder behavior. Dense mappings are verified empty between complete passes. Destination buffers are reused.

The initial parity run is correctness evidence only: other builds/tests were running. The separate quiet timing run is described below. The benchmark includes dense scratch allocation/release per complete split; it does not assume an unbounded cross-frame remap cache. The fast-hash alternative is included in the timing comparison.

## Isolated timing

A CPU-idle guard completes 252 timed runs (12 fixtures × seven repetitions × three algorithms), including dense scratch allocation/release per split. The first repetition is warm-up; the remaining six rotate algorithm order and alternate brick traversal direction. Full sphere remapping medians are 9.89 ms for the original hasher, 6.35 ms for fast hashing and 2.08 ms for dense indices. The 48-Grab sphere measures 9.67, 6.49 and 2.15 ms; the full box 3.84, 2.37 and 0.66 ms. Full vertex bits and indices agree throughout.

Dense remapping is selected for its larger measured gain. The dense table uses approximately 1.13 MB for the largest 141,478-vertex fixture, plus its reusable touched-index list. This is a transient-memory tradeoff; application measurements and production regressions remain required. These component timings do not establish a 16 ms brush action.

## Production regression verification

The release test build succeeds against the committed Core v0.113.0 pin. All 70 library tests pass, including the two new regressions comparing complete vertex bits and index order, reverse brick reuse, invalid placeholders and empty replacements. The informational timing test is ignored. Native agent, stroke latency and settle tests pass. The rendered all-brush image test passes, but the subsequent timing-ratio test fails (Snake Hook median 67.7 ms versus Standard 38.3 ms; required ratio >2) during concurrent compilation. This failure remains unresolved pending an isolated rerun; it is not classified as pre-existing. The runner stopped there; a separate run of the preserved binary subsequently passes all three incremental rendered cases without adapter skips. Thus 86 of 87 enabled cases pass, with the timing-ratio assertion still requiring resolution. Format, Clippy with warnings denied, and all 46 strict OpenSpec items pass. Clippy cognitive-complexity threshold 12 reports no warnings for geometry.rs, including the changed remesh function and new helpers/tests. It reports warnings elsewhere, so this is not a repository-wide complexity-clean claim. Live application timing remains pending.

The guarded rendered rerun was deliberately stopped before launching any test after no quiet window had appeared; the combined build was started instead. All six pinned-engine test executables were preserved for the pending rerun. The combined release build succeeds in 4m 15s against Core `cd215a7a`. The application binary is preserved separately (SHA-256 `1e6cc20ddccdc603078dc4482077e3683e506d17092467a7f2fb77b4c3689e1f`), and the vendor checkout is restored to the committed v0.113.0 pin. Combined correctness and application timing are still in progress.
