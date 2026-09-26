## 1. The gate fails when it cannot compare

- [x] 1.1 `compare` returns an error for a baseline `unlike` declines, and the run exits 2, with a regression test over a baseline that states no scenes
- [x] 1.2 Drop the job's `>= 128` quarantine, which named #34 as its expiry; #34 is closed

## 2. The baseline names its machine

- [x] 2.1 `machine.rs`: processor, logical cores, memory, operating system and runner image, read with safe code on macOS and Linux
- [x] 2.2 Write `conditions.machine` into the baseline and read it back, escaping the processor name, with round-trip tests including a baseline from before the field
- [x] 2.3 Announce a comparison against a baseline recorded on a different processor, core count or memory; announce a baseline that names no machine

## 3. A threshold measured on the runner

- [x] 3.1 `--tolerance-scale K`, refused below 1 or when not a finite number, applied to every figure's tolerance
- [x] 3.2 Measure run-to-run variance from twelve `macos-14` runs and four `ubuntu-24.04` runs at engine 0.120.1 and derive each platform's CI scale from it; record the data in `benchmarks/ci-gate.md`

## 4. Both platforms

- [x] 4.1 The Performance job becomes a macOS and Linux matrix, each against its own baseline, with `--tolerance-scale` 10 on macOS and 3 on Linux
- [x] 4.2 Give `record-baseline` the Linux build dependencies the test job installs
- [x] 4.3 Give both Linux benchmark jobs Mesa's software Vulkan driver, since without an adapter every group that renders or sculpts through the view skips
- [x] 4.4 Record both baselines with the dispatched job and commit its artifacts; keep the 0.52.2 workstation recording under `benchmarks/archive/` for the figures the README cites
- [x] 4.5 Confirm the PR's own Performance runs compare against the new baselines without a false regression, which is the second run of an unchanged tree
- [x] 4.6 Confirm a deliberate slowdown on a scratch branch fails the job

## 5. The intersect regression

- [x] 5.1 Re-measure `object.drag_frame_intersect` against `object.drag_frame` on the current pin, record the result in `benchmarks/ci-gate.md` and the A/B report, and open #282 for the cost that remains

## 6. Say it

- [x] 6.1 README, `docs/architecture.md`, `docs/roadmap.md` and the justfile: which platform gates, at what threshold, and against which baseline
- [x] 6.2 Tick `gates-that-can-fail` 1.5 and `benchmark-every-operation` 8.3
