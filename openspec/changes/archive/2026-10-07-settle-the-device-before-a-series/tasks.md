## 1. Find the sample

- [x] 1.1 Reproduce with the engine CPU-only: clean on `--only brush.voxel.padrao`, stalled on `--only brush` (sample 6 of 13, 80 ms against 15).
- [x] 1.2 Split the sample into phases: 66 of the 80 ms inside `flush_writes`; the ledger reads 341 MB of never-submitted field staging at the voxel series' start.

## 2. Fix the harness

- [x] 2.1 `Screen::prime` flushes, waits for the device and marks it idle (`visible::settle`).
- [x] 2.2 Regression test `a_primed_screen_carries_nothing_the_series_before_left_pending` in the bench binary; fails with the settle removed.

## 3. Make the next one diagnosable from the artifact

- [x] 3.1 `Run` keeps every timed series; `json.rs` writes and reads a `samples` section beside `spread`, additive.
- [x] 3.2 `Stall::find` names a sample more than 3x the next largest and at least 10 ms above it; `Run::timings` announces it.
- [x] 3.3 `CLAYSPACE_BENCH_SAMPLES` prints in the order taken; `CLAYSPACE_BENCH_PHASES` prints each brush sample's phases.

## 4. Record it

- [x] 4.1 `benchmarks/ci-gate.md`: the runs, the cause, the fix and what the artifact now says.
- [x] 4.2 `docs/architecture.md` and `README.md` where they describe the recorded file.
