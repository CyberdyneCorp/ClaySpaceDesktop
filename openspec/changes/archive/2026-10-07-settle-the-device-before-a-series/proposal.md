# Settle the device before a series is timed

No issue; the defect is the performance gate's own. From 30 Sep to 7 Oct every
`Performance` job on both runners tripped or nearly tripped
`brush.voxel.padrao.mean` while the figure's median and p95 held where the
baseline put them (runs 36658810228, 37565433879, 37569795577, 37608653014;
the last clean run is 36635567473 of 29 Sep). `benchmarks/ci-gate.md` has the
table.

## Why

The spread section said one sample in thirteen: 272 ms against 51 on the Linux
runner, 0.8 to 2.0 s against 25 to 45 on the macOS one. The mean carried it;
the median and the second-largest-of-thirteen did not move. The figure read
clean on an Apple M3 Pro, with Metal and with the engine CPU-only, when run on
its own — and stalled as soon as the whole `brush` group ran: sample 6 of 13
at 80 ms against 15 for its neighbours, 66 of them inside `flush_writes`.

The harness never presents a frame. A field series writes its re-meshed bricks
with `write_buffer` and nothing submits them, so wgpu holds every write's
staging as a pending write — 341 MB of it, read off the device ledger, when the
grid's first series began. The grid's route, `set_mesh_layers`, has flushed its
writes in an empty submission since #311, the one commit in the window that
touches a submission. The voxel series' first flush therefore carried the
field series' whole staging and the deferred release of every buffer they had
dropped; the device finished that a few samples in, and the sample whose flush
found it finished paid the release. The application pays the same cost per
frame, spread over frames that submit; a timed series should not pay it at
all.

## What changes

- **A series starts from a settled device.** `Screen::prime` ends by flushing
  the writes, waiting for the device and marking it idle, so what an earlier
  series left pending is paid before the clock starts. Measured on the M3 Pro
  with the engine CPU-only, `brush.voxel.padrao` went from
  `1.79 14.97 15.32 15.10 80.21 15.44 15.22 0.56 ...` to
  `0.59 14.51 14.78 14.81 15.04 14.77 14.75 0.55 ...`.
- **The recorded file keeps the samples.** A `samples` section beside `spread`:
  every sample of every timed measurement, in the order taken, under the
  measurement's prefix. Additive, like `spread`; a file without it still reads.
- **A sample that stands apart is announced.** More than three times the next
  largest in its series and at least ten milliseconds above it, printed as it
  is taken with its position, so the CI report names the sample.
- **Two diagnostic switches.** `CLAYSPACE_BENCH_SAMPLES=1` prints each series
  in the order taken (it printed them sorted, which loses the position);
  `CLAYSPACE_BENCH_PHASES=1` prints each brush sample's phases.

## Out of scope

Field series still never submit their uploads during the series; the settle at
the next arrange carries them. Flushing per field sample would change every
`brush.sdf.*` figure the baselines hold, for a cost the gate has no need of.
