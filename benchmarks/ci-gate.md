# The CI performance gate: what it compares, and at what threshold

Issue #189. CI runs the whole benchmark on two runners and fails when a figure
regresses past a stated threshold, a measured figure goes missing, or the run
cannot compare against its baseline.

| platform | runner | baseline | threshold |
|---|---|---|---|
| macOS | `macos-14` | `baseline-macos-aarch64.json` | tolerance × 10 |
| Linux | `ubuntu-24.04` | `baseline-linux-x86_64.json` | tolerance × 3 |

A figure's tolerance is 1.5 for a mean or a median, 2.0 for a p95 or a one-shot
figure, 1.25 for a count or a size, and each ratio's own. At scale 10 the
macOS gate fails a mean that is **15x** its baseline, a p95 or one-shot figure
at **20x**, and a count at **12.5x**; at scale 3 the Linux gate fails a mean at
**4.5x** and a p95 or one-shot figure at **6x**. On top of the ratio test, the
gate fails on any figure the baseline measured that this run neither measured nor excused, and on a
baseline it refuses to compare against (another scene suite, platform,
architecture or backend). The refusal used to exit 0; it now exits 2.

## Why ten on macOS and three on Linux

Both baselines were recorded by the `Record a baseline` job on the runner image
the gate runs on, and each file's `conditions.machine` says what that was:

| baseline | processor | cores | memory | OS | runner image | load per core |
|---|---|---:|---:|---|---|---:|
| macOS | Apple M1 (Virtual) | 3 | 7 GiB | macOS 14.8.9 | macos14 20260831.0302.1 | 7.99 |
| Linux | Intel Xeon Platinum 8370C @ 2.80GHz | 4 | 15 GiB | Ubuntu 24.04.5 LTS | ubuntu24 20260920.314.1 | 0.96 |

A hosted macOS runner is a three-core virtual machine at a one-minute load of
four to eight per core before the benchmark starts. The Performance job's
artifacts from twelve runs at engine 0.120.1, on 2026-09-26, were compared
pairwise — each run as the baseline for each other, 132 comparisons over 195
shared figures. They are ordinary feature branches (issues #170, #175, #185,
#191, #193, #207, #214) and three pushes to main rather than one tree run
twelve times, so a little of the spread may be real change; that errs towards
a wider threshold, which is the safe direction for a gate that must not flake.

| scale | comparisons with at least one false regression |
|---:|---:|
| 1 (the workstation tolerances) | 129 of 132 |
| 2 | 74 |
| 3 | 30 |
| 4 | 14 |
| 5 | 6 |
| 6 | 2 |
| 7 | 0 |
| 8 | 0 |

The figures that set the floor, as the scale each needed so that no pair
disagreed:

| figure | scale needed |
|---|---:|
| `startup.to_first_document` (one sample) | 6.64 |
| `brush.sdf.pincar.mean` | 6.29 |
| `brush.voxel.vinco.mean` | 5.03 |
| `brush.sdf.pincar.p95` | 4.71 |
| `multires.pass_stroke.p95` | 4.16 |

The whole-suite geometric mean of one run over another ranged from 0.64x to
1.57x: the runner, not the code, is most of the variance.

The committed baseline is a thirteenth run, and a single run lands wherever the
runner put it: its `brush.sdf.pincar.mean` is 145 ms, where the twelve read
between 172 and 1,628 ms. Held against it, the twelve need a scale of **7.46**
(that figure, in the 1,628 ms run) before every one passes. Ten is that with a
third to spare. It is coarse, and it is what a hosted Mac supports; a
self-hosted macOS runner would be the way to tighten it. This pull request's
own macOS Performance job — the same tree as the recording, run again on
another runner — needed 1.11 against the committed file
(`brush.mesh.inflar.p95`, 12.70 → 28.27 ms), so a run that lands close to the
recording is the common case and the 7.46 is the tail.

The Linux runners are a different kind of machine: four cores, 15 GiB, a load
of about one per core, and no GPU, so the view renders through Mesa's software
Vulkan driver (lavapipe), which is slow and the same every run. Four whole
runs of the measured code were available when this was written, each landing
on a different processor: the recording itself (Intel Xeon Platinum 8370C),
the Performance job of the same dispatch (AMD EPYC 7763), this pull request's
own Performance job (AMD EPYC 9V45), and a scratch branch whose only change was
in code the benchmark never reaches (AMD EPYC 9V74). Compared pairwise in the
direction the gate tests, the worst figure was `locality.dab_ms_10x` at 6.62 ms
on the 9V45 against 18.74 ms on the Xeon, which needed a scale of **1.89**;
held against the committed baseline, no run needed more than 1.0. Three is
1.89 with room for processors not yet seen, and keeps the several-fold
regressions the macOS row cannot see. If a Linux run ever disagrees with its
own baseline on an unchanged tree, that is the evidence to raise it, and this
file is where to put it.

**The gate fails when it should.** A scratch branch that put a 100 ms sleep at
the top of `ClayDocument::apply_stroke` — every brush, every stroke — was run
through CI (run 36258612853). The Linux Performance job failed with 51 figures
marked `REGRESSED`, every mesh brush among them at about 6x its baseline
(`brush.mesh.padrao.mean` 18.94 → 116.31 ms), and exited 1. The same branch
with the sleep in a function the benchmark never reaches passed, which is the
fourth Linux run above.

**What this gate does and does not catch.** It catches the regressions that
matter most and are easiest to ship without noticing: a figure that goes up by
an order of magnitude (the chain-growth problem in `move-segment-cost.md` is
2 → 40.7 ms, 20x), a group that starts refusing its edit, and a measurement
that quietly stops running. It does not catch a 1.2x regression on one brush.
That is what the workstation tolerances are for, on a quiet machine: record
with `just bench-to`, change, compare with `just bench-against`.

## Recording now, and re-recording

These baselines are recorded **before** the performance work in epic #150, at
engine 0.120.1 (CyberRemesher 0.10.0), as a floor. Recording after the epic
would leave both platforms ungated for its whole length, and the thing the gate
most needs to catch — something getting worse — is as possible during the epic
as after it. Each change that makes a figure substantially better re-records
the baselines in its own commit with the reason, so the gate then holds the
better number:

```sh
gh workflow run ci.yml -f record_baseline=true   # on the branch being merged
# download the baseline-macos-aarch64 and baseline-linux-x86_64 artifacts
# into benchmarks/, commit with the reason
```

## The intersect regression

The v0.73 → v0.78 A/B left `object.drag_frame_intersect` 1.166x slower, with
its subtract control flat. The absolute figure cannot be compared across
machines, but the ratio of the two within one run can:

| pin | where | intersect ÷ subtract |
|---|---|---:|
| v0.73.0 | Linux reference, CUDA | 2.25x |
| v0.78.0 | Linux reference, CUDA | 2.59x |
| v0.84.0 | Linux reference, CUDA | 2.54x |
| v0.120.1 | `macos-14`, Metal, twelve runs | 2.73x median, 1.80–3.75 |

The A/B report's addendum traced it to an intersect's influence bound being its
whole layer, so every drag frame refilled the layer. Since #282 an object move
refills the overlap of that bound with the region the engine says the move
changed (`clay_layer_set_transform_bound`). That region is the sweep, dilated
by the layer's chain pad. In brick keys per frame
(`crates/clayspace-app/tests/intersect_drag_scaling.rs`), the intersect went
from 5,040 to 3,360 on `reference` and from 84,672 to about 25,000 on
`reference-10x`, against 1,012 subtracting. The rest is the engine's pad,
CyberdyneCorp/ClayCore#666. Both committed baselines
carry `object.drag_frame_intersect`, recorded before the change, so the gate
holds it to the old figure until they are re-recorded.

## The stalled voxel dab

From 30 Sep to 7 Oct every Performance job, on both runners, failed or nearly
failed `brush.voxel.padrao.mean` while the figure's own median and p95 sat
where the baseline put them. The spread section said why: one sample in
thirteen.

| run | tree | runner | mean | min / median / p95 / **max** (ms) |
|---|---|---|---:|---|
| 36635567473, 29 Sep | `db9a000` (#310) | `macos-14` | 19.1 | 0.81 / 13.6 / 39.4 / 48.9 |
| same | same | `ubuntu-24.04` | 25.2 | 2.23 / 2.27 / 52.1 / 52.3 |
| 36658810228, 30 Sep | `b0d54a8` (#315) | `macos-14` | 74.2 | 0.99 / 25.9 / 32.3 / **772** |
| same | same | `ubuntu-24.04` | 45.3 | 1.70 / 51.0 / 51.3 / **272** |
| 37569795577, 7 Oct | #316 | `macos-14` | 145.5 | 4.45 / 27.4 / 43.3 / **1,652** |
| same | same | `ubuntu-24.04` | 40.7 | 1.44 / 50.5 / 50.8 / **216** |
| 37608653014, 7 Oct | `0999dc6` (#316 on main) | `macos-14` | 165.4 | 1.01 / 24.5 / 30.1 / **1,968** |
| same | same | `ubuntu-24.04` | 46.9 | 1.68 / 51.9 / 52.4 / **286** |

The Linux baseline holds the mean at 25.16 ms; 4.5x of that is 113 ms, and a
single 270 ms sample in a thirteen-sample mean adds 17 ms. The median hid it,
the p95 (the second largest of thirteen) hid it, and the mean carried it. The
figure read clean on an Apple M3 Pro with Metal (`--only brush.voxel.padrao`,
max 15 ms) and clean on the same machine with the engine CPU-only — and
stalled, sample 6 of 13 at 80 ms against 15 ms for its neighbours, as soon as
the whole `brush` group ran: it needed the twelve field brushes to run first.

**The cause was in the harness.** The benchmark never presents a frame. A field
series writes its re-meshed bricks with `write_buffer` and nothing submits
them, so wgpu holds every write's staging as a pending write: 341 MB of it,
read off the device ledger, when the grid's first series began. The grid's and
the mesh's route is `set_mesh_layers`, which has flushed its writes in an empty
submission since #311 (`18b7ce3`, the one commit in the window that touches a
submission). The first flush of the voxel series therefore carried the field
series' whole staging and the deferred release of every buffer they had
dropped; the device finished that a few samples later, and the sample whose
flush found it finished paid the release — 66 of its 80 ms were inside
`flush_writes`, with the engine's dab, the re-smooth, the meshing and the
vertex build all at their usual cost. Before #311 the same staging leaked
instead of being carried, which is the growth #311 was fixing; the run of
29 Sep is the last before it.

Every series now starts from a settled device: `Screen::prime` flushes the
writes, waits for the device and marks it idle, so what an earlier series left
pending is paid before the clock starts, as the application pays it per
frame. Measured on the M3 Pro CPU-only, `brush.voxel.padrao` went from
`1.79 14.97 15.32 15.10 80.21 15.44 15.22 0.56 ...` to
`0.59 14.51 14.78 14.81 15.04 14.77 14.75 0.55 ...`, and the ledger at the
series' start from 341 MB to 7 MB. `brush.voxel.suavizar.ms` had the same
stall in one of its three rebuilt samples on macOS (1,328 and 2,121 ms against
30 and 46), hidden by its median; it goes with it. The bench's own test
`a_primed_screen_carries_nothing_the_series_before_left_pending` fails with
the settle removed.

**What the artifact now says.** Three things, so the next sample that stands
apart is identified by the run that took it rather than reproduced a week
later:

- `now.json` carries a `samples` section beside `spread`: every sample of every
  timed measurement, in the order taken, under the measurement's prefix.
- A sample more than three times the next largest in its series, and at least
  ten milliseconds above it, is announced in `report.txt` as it is taken —
  `brush.voxel.padrao: sample 6 of 13 took 272.05 ms; the next largest took
  51.28`. On the Linux runner of 29 Sep no repeatable figure's largest sample
  exceeded 1.25x its second largest, so on that runner the line means
  something; the loaded macOS runner produces one or two a run on its own.
- `CLAYSPACE_BENCH_SAMPLES=1` prints each series as it is taken, and
  `CLAYSPACE_BENCH_PHASES=1` prints each brush sample's phases (the engine
  edit, the re-smooth, the meshing, the mask, the vertex build and the
  upload), which is how the flush was named.
