# Validation

## Final scope and revisions

Retain direct mapped uploads only for compaction after an ordinary edit. Full rebuilds, preview initialization and ordinary relayout use the original per-brick uploads. Local patches are unchanged.

Baseline desktop: `895cac8`. Both applications use ClayCore `86f2ad9c` (PR #623), ahead of the desktop vendor pin, with the same default backend selection and release build. The engine gitlink is not part of this change; local vendor symlinks are excluded from the commit. These results do not validate the published desktop engine pin or another device.

Hardware: Linux x86_64, Intel Core i9-12900K (24 logical CPUs), NVIDIA RTX 5060, driver 580.95.05. All 780 measured phases reported backend `cuda`.

## Correctness and maintainability

Final release run, without retries: application library 81 passed / 3 ignored; agent end-to-end 4 passed; mapped GPU upload 1 passed; sculpt latency 5 passed; settlement 5 passed; visual sculpting 18 passed. Total: **114 functional tests passed**. The informational upload benchmark was run separately.

Regressions cover complete vertex attribute bits, slot/index layout, degenerate tails, live-only bounds, empty keys, incremental growth/relocation, and capacity refusal. Direct writers match an independent CPU-array reference in dirty, deliberately unaligned byte slices, including NaN payloads and signed zeros. The GPU test verifies exact rendered pixel equality, a visibly nonempty reference, upload accounting, and empty callbacks. The routing test verifies sparse full-rebuild byte counts and mapped compaction payloads.

Strict Clippy (`--lib --tests -- -D warnings`), formatting, whitespace and all 53 strict OpenSpec checks pass. Rust is unsupported by the cognitive-complexity skill; Lizard cyclomatic fallback scores for new production functions are 1–4. These are not cognitive scores.

Build: `CARGO_TARGET_DIR=/tmp/clay-531-host-target CARGO_BUILD_JOBS=4 LD_PRELOAD=/usr/lib/x86_64-linux-gnu/libstdc++.so.6 cargo build --release -p clayspace-app --bin clayspace-app`.

Tests: `cargo test --release -p clayspace-app --features agent-e2e --lib --test mapped_upload --test agent_end_to_end --test sculpt_latency --test settle_needed --test visual_sculpting --no-run`, then run each executable serially with `--nocapture --test-threads=1`, a real GPU/display and `CLAYSPACE_AGENT_E2E=1`. End-to-end tests use the preserved default-feature application.

## Isolated upload experiment

Real starting SDF sphere: 2,744 stored keys, 161,786 vertices, 281,520 triangles. One GPU process, three warm-up rounds, 30 measured rounds per variant, rotating order. Queue submit/drain occurs outside the measured interval. This measures upload CPU work, excluding meshing, pruning, transfer completion and presentation.

| Upload implementation | Median ms | Uploaded bytes |
|---|---:|---:|
| Original per-brick | 2.546 | 10,982,672 |
| First CPU-array prototype | 3.810 | 13,473,632 |
| Direct mapped | 1.506 | 13,473,632 |

Direct mapping was faster than per-brick in all 30 rounds; paired median reduction was 1.065 ms. This is about 41% less isolated upload CPU time, **not** a 41% whole-brush improvement. Raw samples: `evidence/upload-comparison.csv`. Reproduce with the ignored `geometry::tests::profile_whole_surface_uploads` test in release mode, alone on a quiet host.

The first prototype's roughly 21 MiB CPU staging-array capacity is removed. This memory reduction is relative to the prototype, not original main. Mapped compaction still transfers roughly 23% extra bytes through vertex headroom; wgpu still allocates staging storage.

## Whole-application comparison

Ten alternating baseline/candidate pairs, 13 brushes per process, 260 brush cases / 780 phase measurements. Before each brush, undo history restores the starting sphere. Select the tool, begin at `[0,0,1]` with pressure 1, continue at `[0.12,0,1]`, and end. CPU affinity: 0,2,4,6,8,10,12,14.

Wait for three one-second samples with CPU idle >=75% and load average <5 before starting, and again after each application startup. Startup is excluded from timing. Monitor CPU every 250 ms and reject four consecutive samples below 40% idle during measurements. No local builds, tests or profilers run concurrently. The completed run passed this guard; it cannot eliminate per-core, driver, GPU or frequency noise. The live `measure` command covers synchronous brush work and queued upload accounting, not input-to-display latency.

Final selective results; paired delta is candidate minus baseline within each pair:

| Action | Baseline median ms | Candidate median ms | Paired median delta ms | Faster pairs |
|---|---:|---:|---:|---:|
| mask end | 8.05 | 8.18 | -0.02 | 6/10 |
| crease end | 13.19 | 12.28 | -1.38 | 8/10 |
| clay end | 13.84 | 13.11 | -0.91 | 7/10 |
| inflate end | 13.58 | 12.43 | -1.69 | 9/10 |
| layer end | 13.34 | 11.81 | -1.50 | 9/10 |
| standard end | 13.15 | 12.15 | -1.12 | 7/10 |
| polish end | 31.48 | 30.23 | -1.22 | 8/10 |
| planar end | 31.46 | 30.08 | -1.54 | 8/10 |
| move-topological end | 43.35 | 42.19 | -1.43 | 7/10 |
| move end | 33.82 | 34.63 | +0.04 | 5/10 |
| relax begin | 50.51 | 49.43 | -0.80 | 6/10 |
| relax end | 57.96 | 57.70 | +0.05 | 5/10 |
| smooth begin | 50.28 | 48.59 | -0.96 | 8/10 |
| smooth end | 59.68 | 57.77 | -1.10 | 7/10 |
| snake-hook end | 23.46 | 22.38 | -0.82 | 8/10 |

Ordinary releases show a repeatable improvement of roughly 0.7–1.5 ms in medians. Full rebuild paths have identical upload payloads: e.g. Smooth/Relax release remains 10,982,672 bytes. Changes on those unchanged paths should not be credited to this optimization. Move release has a higher unpaired median but a +0.04 ms paired median and five faster/five slower pairs; Relax release is similarly mixed. Clay includes a candidate 23.36 ms outlier. Ten pairs establish a useful local improvement, not a tail-latency guarantee.

Raw observations and all phase summaries are retained in `evidence/selective-direct.jsonl` and `evidence/selective-direct-summary.json`. No outliers were removed. Smooth/Relax still take about 50–60 ms; several other releases also exceed 16 ms. **Issue #531 remains open.**

Executable SHA-256:

- Baseline: `4871e14665f25b5c5a6cc3cde9ed25181f92452eebfc86cb71bfaddd92c4cfbc`
- Final selective candidate: `122c4bd4150c92e8c7ea5e716ffc3f2cb2dec9d9b03f80cc83ed47780e824b56`

## Why full rebuilds retain sparse uploads

A separate complete ten-pair comparison of universal direct staging improved some ordinary releases but regressed Smooth begin from 49.779 to 53.400 ms, slower in all ten pairs. Its exact cause was not established. Retain the observations in `evidence/universal-direct.jsonl` and its summary. Restricting the optimization to compaction removes that consistent regression in the final comparison without brush-name dispatch.

## Earlier prototype history

`evidence/pilot.json` retains the first CPU-array prototype's incomplete one-pair pilot. It stopped on a later application startup timeout and did not establish a latency win. Its correctness run required a consent-test retry; the main application also failed in the consent section at a different assertion. Neither that pilot nor its retry is counted as final acceptance. The final implementation's separate 114-test run passed without retries.
