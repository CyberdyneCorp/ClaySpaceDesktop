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
self-hosted macOS runner would be the way to tighten it.

The Linux runners are a different kind of machine: four cores, 15 GiB, a load
of about one per core, and no GPU, so the view renders through Mesa's software
Vulkan driver (lavapipe), which is slow and the same every run. The two whole
runs available when this was written — the recording itself, on an Intel Xeon
Platinum 8370C, and the Performance job of the same dispatch, on an AMD EPYC
7763 — agreed on all 206 figures to within the workstation tolerances except
one, `locality.dab_ms_10x` (18.74 → 10.61 ms), which needed a scale of 1.18.
Three is that with room for the processors a hosted runner may be given,
without giving up the several-fold regressions the macOS row cannot see. If a
Linux run ever disagrees with its own baseline on an unchanged tree, that is
the evidence to raise it, and this file is where to put it.

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

It has not gone away. It is not unexplained either: the A/B report's
addendum traced it to an intersect's influence bound being its whole layer, so
every drag frame refills the layer, and that is now issue #282 with the figure
to watch. Both committed baselines carry `object.drag_frame_intersect`, so it
cannot quietly get worse.
