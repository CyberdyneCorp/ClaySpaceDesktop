# Gate performance on both platforms

## Why

Issue #189. A performance regression could not be caught on the platform the
application is developed on, and not reliably on the other one either:

- The macOS baseline was recorded on engine 0.29.1 and predates the reference
  suite. `compare::unlike` declined it, `compare` returned "nothing regressed",
  and the Performance job went green on every commit.
- The Linux baseline was recorded on engine 0.52.2, on a developer's CUDA
  workstation, and no CI job compared against it at all. It also still named
  `voxel-reference-r1` after the suite moved to r3.
- Neither file said what machine produced it beyond platform, architecture and
  backend, so a comparison between a workstation and a three-core runner could
  not tell its reader why every figure moved.
- The v0.73 → v0.78 A/B left a live-intersect regression "unexplained".

## What changes

- **A refusal to compare fails.** `compare` returns an error for a baseline it
  declines, and the run exits 2. The job step no longer tolerates a signal
  either: the quarantine for #34 expired with #34.
- **The baseline names its machine.** `conditions.machine` records the
  processor, logical cores, memory, operating system and, on a hosted runner,
  the runner image and version. A comparison against a baseline recorded on a
  different processor, core count or memory says so above the table.
- **`--tolerance-scale K`** multiplies every figure's tolerance. Absent, it is
  1 — the tolerances the reference machine was measured to allow. CI passes 10
  on macOS, measured from twelve runs of the suite on `macos-14` at the current
  pin: a hosted Mac moves single figures by up to 10x between runs, and a scale
  of 6.6 was the smallest that made no pair of those runs disagree (7.5
  against the committed baseline itself). Linux
  passes 3: four runs on four different processors needed at most 1.89.
- **The Performance job runs on macOS and Linux**, each against a baseline
  recorded by the `Record a baseline` job on the same runner image and
  committed to `benchmarks/`. The recording job gained the Linux build
  dependencies it lacked (it could not have built on Linux), and both Linux
  jobs install Mesa's software Vulkan driver: the runner has no GPU, and
  without an adapter the first Linux run skipped every group but startup,
  memory and a handful of CPU-only figures.
- **The intersect regression is re-measured** on the current pin: the
  intersect ÷ subtract ratio is 2.73x (median of twelve runs), against 2.25x at
  v0.73.0. The mechanism is known — an intersect's influence bound is its
  layer, so every drag frame refills the whole layer — and it is tracked as
  #282.

## Decision: record now, re-record after the epic

The baselines are recorded now, on the current pin, as a floor. Several issues
in epic #150 change performance substantially; each of them re-records the
baseline in its own commit with the reason, and the gate meanwhile catches
anything that makes the current state worse. Waiting for the epic to land would
leave both platforms ungated for its whole duration.

## Impact

- `crates/clayspace-app/src/bin/bench/`: `machine.rs` (new), `compare.rs`,
  `json.rs`, `figures.rs`, `main.rs`.
- `.github/workflows/ci.yml`: the Performance job becomes a two-platform
  matrix; `record-baseline` installs the Linux dependencies.
- `benchmarks/`: both baselines re-recorded on the runners; the 0.52.2
  workstation recording kept under `benchmarks/archive/` because the README's
  figures cite it; `ci-gate.md` records the noise measurement and the intersect
  re-run.
- Docs: README, `docs/architecture.md`, `docs/roadmap.md`, `justfile`.
