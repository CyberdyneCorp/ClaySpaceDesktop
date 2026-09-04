# ClayCore v0.73.0 → v0.78.0, measured

Ten full benchmark runs — five on each pin, interleaved — taken on 2026-09-03
between 15:55 and 16:43 local time on the Linux reference machine.

**The headline is that nothing moved.** The median of 178 shape-matched figures
is **0.9998x**. Seven of those 178 moved by more than their own run-to-run
spread: four got faster, three got slower. The remaining 171 are figures whose
ratio the data cannot separate from noise, and they are reported as exactly
that rather than as small wins and small losses.

That is the same result the release notes report from the reference iPad
(0.9949x over 298 points), reached independently on different hardware, a
different backend and a different harness. **A release that adds 146 entry
points and a whole surface tier costs the existing verbs nothing measurable
here either.**

Two things in this report are not in the vendor's: a regression on live
intersecting booleans that runs against one of the release's own claims, and a
marked improvement in *stability* on mesh brushes that no median can show.

---

## Conditions

| | A (before) | B (after) |
|---|---|---|
| worktree | `/home/leonardo/work/clayspace-ab-base` | `/home/leonardo/work/ClaySpaceDesktop` |
| commit | `bcc78ad` | `2e6904a` (branch `engine-0-78-0`) |
| ClayCore | **v0.73.0**, submodule `05d1fe08` | **v0.78.0**, submodule `512c8c5d` |
| engine as the harness recorded it | `0.73.0`, revision not recorded | `0.78.0` (`v0.78.0-0-g512c8c5d`) |
| bench binary | `target/release/bench`, built 00:50, **not rebuilt** | `target/release/bench`, rebuilt before measuring |

A records no engine revision because the field did not exist on that pin; the
harness reads it back as `None` and says "revision not recorded" rather than
inventing one. That asymmetry is the only difference between the two
`conditions` blocks apart from the engine version itself. Everything `compare.rs`
actually gates on is identical on both sides:

```
scenes      mesh-reference-r1, reference-r1, reference-10x-r1,
            voxel-pocked-r1, voxel-reference-r1
platform    linux          architecture  x86_64
backend     cuda           viewport      1280x800
```

**Machine.** 12th Gen Intel Core i9-12900K, 24 logical cores (8 performance + 8
efficiency), NVIDIA GeForce RTX 5060 (8 GiB, driver 580.95.05), Linux Mint 22.1
on kernel 6.8.0-90-generic.

**Load, and the honest version of it.** The machine was *not* idle. Three
workloads outside this measurement were resident throughout: a runaway process
left by an unrelated session pinning one core at 99% for the whole 29 hours
around the campaign, a long-running C++ integration test at ~13% of a core, and
a browser. Other sessions on the box also started and finished C++ builds during
the window, which is what the load spikes below are.

The harness samples the one-minute load *before* its warm-up, precisely because
a benchmark is most of its own load once running. Per core, as recorded in each
run's own JSON:

| | run 1 | run 2 | run 3 | run 4 | run 5 | median |
|---|---:|---:|---:|---:|---:|---:|
| A | 0.400 | 0.240 | 0.356 | 0.292 | 0.132 | 0.292 |
| B | 0.175 | 0.245 | 0.225 | 0.190 | **0.463** | 0.225 |

The harness calls anything under 0.25 per core quiet and refuses to record a
baseline at 0.5 or above. Every one of the ten runs was under that refusal
threshold — none needed `--allow-busy`, and none was discarded — but seven of
ten sit in the warn band. **The figures below were measured against other work
on this box.** That is the single largest caveat on this report, and it is the
reason the campaign was interleaved and the reason a ratio is only called real
when the two sides' five-run ranges do not overlap at all.

---

## Method

1. **The machine was checked, not assumed.** Load and process table inspected
   before starting. The competing work described above was characterised rather
   than waited out: it had already run for hours and showed no sign of ending,
   and the residents that mattered were roughly constant across the window,
   which is what interleaving is for. The load was recorded per run.
2. **B was rebuilt** with `cargo build --workspace --release`. **A was not
   touched** — not rebuilt, not modified, submodule left at v0.73.0. Verified
   clean before and after.
3. **One throwaway run of each side** was taken first and discarded, to learn
   the runtime and to warm the filesystem caches equally on both sides. Those
   two produced no JSON (their stdout was piped to `head`, which broke the pipe
   and killed the table print before the file was written) and are not used.
4. **Ten full runs, strictly alternating: A, B, A, B, …** five of each, each
   with `--json` to its own file, each writing its own stdout log. No `--only`:
   a filtered run refuses to record and a partial set is not comparable.
5. **Thirty seconds of settling between runs.** A run takes about four minutes
   on A and four minutes ten on B; the campaign took 48 minutes end to end.
6. Any run exiting non-zero or writing no JSON would have been discarded and
   retaken, up to three attempts. **None was: all ten runs succeeded on the
   first attempt.**

Raw material is under
`/tmp/claude-1000/-home-leonardo-work-ClaySpaceDesktop/f55d797f-16cf-4a2d-bd35-f9e7bfc11780/scratchpad/ab/`
— ten JSON files, ten logs, and the analysis.

### Which route the comparison took, and why

**The harness would not have refused this comparison.** `compare::unlike`
gates on scenes, platform, architecture and backend, and those are identical
here. It deliberately does *not* gate on the engine — its own doc comment says
that a comparison across two pins is the whole point of an upgrade measurement
and that refusing it would leave the one question the gate is best placed to
answer with no instrument — and `across_engines` announces the difference above
the table instead.

So the ratios here were **not** computed in Python to get around a refusal.
They were computed in Python because `compare.rs` compares *one* run against
*one* baseline, and the question this campaign was run to answer needs five
against five: a median per side, and a spread per side to hold the ratio
against. The script is
`/tmp/claude-1000/-home-leonardo-work-ClaySpaceDesktop/f55d797f-16cf-4a2d-bd35-f9e7bfc11780/scratchpad/analyse.py`.

### What "beyond its own spread" means in the last column

It means the two sides' five observations are **completely disjoint** — every B
run outside the range of every A run, or the reverse. With five observations a
side that is the strongest ordering the data can express, and it arises by
chance with probability 2/C(10,5) = **0.0079**.

Anything short of disjoint is reported as `no`, **however large the ratio**. A
figure at 1.11x whose sides overlap has told us nothing, and this report says
nothing rather than calling it a regression. That is why 171 of 178 rows say
`no`: not because the two pins are identical on those figures, but because five
runs a side on a shared machine cannot resolve them.

---

## The headline

| | |
|---|---:|
| shape-matched figures | **178** |
| median ratio B/A | **0.9998x** |
| within ±5% of parity | 128 of 178 |
| moved beyond spread — **faster** | **4** |
| moved beyond spread — **slower** | **3** |
| ratio could not be separated from noise | **171** |
| B-only figures, reported as measurements not ratios | **23** |
| A-only figures | 0 |

Quartiles of the ratio distribution: p25 **0.985x**, median **0.9998x**, p75
**1.013x**. The tails are 0.422x and 1.326x and both are discussed below.

### Everything that moved beyond its own spread

| figure | A median | A range | B median | B range | B/A | |
|---|---:|---|---:|---|---:|---|
| `brush.mesh.planar.ms` | 60.4 | 29.1–86.6 | 25.5 | 24.9–25.7 | **0.422x** | faster |
| `brush.mesh.relaxar.ms` | 48.0 | 46.4–129.3 | 41.9 | 41.6–42.5 | **0.872x** | faster |
| `brush.mesh.suavizar.ms` | 45.9 | 44.9–174.4 | 42.2 | 41.9–44.2 | **0.919x** | faster |
| `render.1080p.gpu.depth_reduce` | 0.0739 | 0.0731–0.0750 | 0.0717 | 0.0713–0.0727 | **0.970x** | faster |
| `object.pick.ms` | 0.100 | 0.0961–0.1014 | 0.113 | 0.1022–0.1356 | **1.133x** | slower |
| `object.drag_frame_intersect.mean` | 57.3 | 56.2–58.8 | 66.8 | 65.4–97.0 | **1.166x** | slower |
| `object.drag_frame_intersect.p95` | 58.9 | 55.1–64.9 | 69.3 | 65.2–76.3 | **1.176x** | slower |

The three "faster" mesh brushes are disjoint for a reason worth reading
carefully, and it is not that the brush got faster. See *Stability* below.

---

## What regressed

### Live intersecting booleans, and this one is real

`object.drag_frame_intersect` is one frame of a live boolean drag: a placed
`Shape::Cylinder` (radius 0.25, height 1.6) dragged across the reference form
with `Combine::Intersect`, the boolean re-evaluated every frame. It is measured
beside `object.drag_frame`, which is **the same fixture, the same drag, the same
frame path, differing only in the operation** — that one subtracts.

| | A (v0.73.0) | B (v0.78.0) | B/A |
|---|---:|---:|---:|
| `object.drag_frame.mean` — **subtracting** | 25.49 ms | 25.82 ms | 1.013x (noise) |
| `object.drag_frame_intersect.mean` — **intersecting** | 57.35 ms | 66.84 ms | **1.166x** |
| intersect ÷ subtract, within one pin | **2.25x** | **2.59x** | |

Per run, in campaign order:

```
A  56.22  57.35  57.12  57.36  58.84     (spread 1.05x)
B  65.37  66.84  65.52  97.02  68.23
```

**The subtracting arm is flat and the intersecting arm is not.** Since both arms
share the fixture, the drag, the GPU work and `screen.refresh`, the frame path
cannot be what moved — if it were, the subtract would have moved with it. What
is left is the operation.

B's fourth run carries a 97.02 ms outlier. Drop it and B is 65.4–68.2 against
A's 56.2–58.8: still completely disjoint, still about 1.16x. The finding does
not depend on the outlier.

Is it real or is it the machine? **Real, as far as this campaign can tell.** The
sides are disjoint with no overlap at all; A's own five runs span only 1.05x, so
this is not a figure that wanders; the runs were interleaved, so the two sides
saw the same machine minutes apart; and the load medians are on B's side of the
argument, not A's — B's median load per core was *lower* (0.225) than A's
(0.292). A busier machine measuring B could have manufactured this; a quieter
one cannot.

What this campaign **cannot** settle is the mechanism. The figure is a whole
frame, not the engine's `plan()` in isolation, and this harness has no
instrument that isolates the cull. Someone should reproduce it against
`plan()` directly on the engine side before anything is concluded about cause.

### Object picking, technically real and probably not worth acting on

`object.pick.ms` is disjoint — but by one microsecond. A spans 0.0961–0.1014 ms,
B spans 0.1022–0.1356 ms; the gap between `max(A)` and `min(B)` is 0.0008 ms.
The ratio is 1.133x of a tenth of a millisecond, on a call the application makes
only on a press. B's spread (1.33x) is also more than three times A's (1.06x),
which is the shape of a figure that has started wandering rather than one that
has moved.

**Called real by the criterion and not worth acting on by any other reading.**
It is recorded so that the next campaign can see whether it stays.

### The two large tails, both of which are noise

`convert.mesh_to_voxel.ms` reads 2.43x slower if you compare only the first run
of each side — which is what a single-observation A/B would have done, and it
would have been the loudest finding in this report. Across five:

```
A  1416  1450  1409  1454  1493      median 1450
B  3436  1397  1413  1408  5113      median 1397     ratio 0.975x
```

Three of B's five runs are *faster* than every A run; two are wild. Both wild
ones are B1 and B5 — the first run of B and the run that started at the
campaign's highest load, 0.463 per core. **This figure is the single best
argument for having run repeats at all**, and it is reported as unsettled.

`mask.ungated.mean` (1.122x) is the same story with a single 20.4 ms excursion
in B2 against a 3.5–3.8 ms baseline, and `render.2160p.ao_off.frame.p95`
(0.889x) the same on A's side.

### Figures that look like regressions and are not settled

Reported here so they are not lost, all with overlapping ranges:

| figure | B/A | A spread | B spread |
|---|---:|---:|---:|
| `mask.ungated.p95` | 1.326x | 1.42x | 6.73x |
| `history.edit.p95` | 1.164x | 1.25x | 1.46x |
| `history.undo.p95` | 1.142x | 1.06x | 1.38x |
| `mask.gated.p95` | 1.128x | 1.14x | 5.64x |
| `dab.median` | 1.113x | 1.18x | 1.14x |
| `dab.p95` | 1.107x | 3.34x | 1.39x |
| `brush.sdf.argila.p95` | 1.104x | 1.22x | 1.09x |

`dab.median` at 1.113x with both spreads near 1.15x is the archetype: it looks
like an 11% regression and the data cannot tell it from nothing. It is also, as
it happens, the figure closest in shape to one of the release's own claims —
see below.

---

## Stability: the finding the median cannot show

The three mesh brushes in the "faster" list are not faster because the brush got
cheaper. They are disjoint because **A jitters and B does not**:

```
brush.mesh.planar.ms
  A   60.41   29.25   29.12   79.63   86.57        spread 2.97x
  B   24.93   25.70   25.73   25.49   25.34        spread 1.03x

brush.mesh.camada.mean
  A   21.33   17.20   16.79   26.51   69.57        spread 4.14x
  B   17.15   17.22   17.26   16.99   17.20        spread 1.02x

brush.mesh.raspar.mean
  A   15.27    9.38    9.07   24.27   25.79        spread 2.84x
  B    9.32    9.50    9.11    9.33    9.33        spread 1.04x
```

**Interleaving is what makes this readable.** A4 ran at 16:22 and B4 at 16:27,
five minutes apart on the same machine; A4's `planar` is 79.63 ms and B4's is
25.49 ms. A5 ran at 16:32 and B5 at 16:37 — and B5 began at the campaign's
*highest* recorded load, 0.463 per core — yet A5 reads 86.57 ms and B5 reads
25.34 ms. Machine drift cannot produce that pattern, because the drift would
have hit the run that came after just as hard.

Across all 163 figures that varied at all: **A's spread is materially wider on
52 of them, B's on 16.** Median within-side spread factor is **1.162x for A**
and **1.097x for B**.

The counterexample is honest: the `mask.*` figures and `bake.export.ms` are
markedly *less* stable on B (5–7x spreads against A's 1.1–1.4x), and
`brush.mesh.pincar` is bad on both. B is steadier on mesh brushes specifically,
not everywhere.

This report does not claim a cause. It is consistent with the maintenance queue
and the quality/decay machinery the release added around mesh sculpting, and it
is also consistent with an allocator or a cache behaving differently; nothing
measured here distinguishes those. **What is defensible is the observation: on
this harness, mesh brush timings on v0.78.0 are repeatable in a way they were
not on v0.73.0.** For a sculpting application that is arguably worth more than a
median, because a brush that occasionally costs three times its usual is felt
and a brush that is uniformly 4% slower is not.

---

## Do the release's own claims show up here?

Four claims, measured against them directly. **Two do not appear, one appears
and is unsettled, and one is contradicted.**

### #319 — "an intersect is bounded by its layer" — **contradicted**

This is the claim this harness is best placed to test, because it already has
the experiment. The reporter measured "the same object, the same drag, the same
scene, differing only in the operation: **19.39 ms subtracting against 35.52 ms
intersecting**" — a ratio of **1.83x** before the fix — and #319 bounds the
intersect so that it refills the box the shape reaches rather than the whole
cache.

`object.drag_frame` and `object.drag_frame_intersect` are that experiment, and
the bench's own module doc describes the very defect #319 names: "an ordinary
cube placed with `Intersect` dirties the whole layer every frame while the same
cube subtracting dirties its own box."

| | subtracting | intersecting | ratio |
|---|---:|---:|---:|
| reporter, before #319 | 19.39 ms | 35.52 ms | 1.83x |
| **A here (v0.73.0)** | 25.49 ms | 57.35 ms | **2.25x** |
| **B here (v0.78.0)** | 25.82 ms | 66.84 ms | **2.59x** |

**The gap did not close. It widened**, and the absolute intersect cost is 1.166x
of what it was, disjointly.

The release notes carve out three cases that keep `Everything` and are correctly
*not* bounded: spatial morphs, an infinite grid repeat, and an unbounded
primitive. **This fixture is none of them** — it is a finite `Shape::Cylinder`
placed with `Combine::Intersect`, which is exactly the case #319 says it now
bounds. So "the exclusion applies here" is not available as an explanation.

Two readings remain, and this campaign cannot choose between them: the bound is
not reaching this path in the way the application drives it, or it is reaching
it and something else in the frame got more expensive by more than the bound
saved. Either way, **the improvement is not visible from this application, and
the figure moved the wrong way.** This is the one item in this report that
deserves a human's attention.

### #441 — 5.3x on `plan()` at 50,000 items — **no instrument, cannot say**

Nothing in this harness plans over 50,000 items; the reference scene's dab
touches 12 keys over 1,049 surface bricks. There is no figure here that
measures `plan()` in isolation at any item count, so this campaign can neither
confirm nor deny it. **Absence of the improvement in this report is not evidence
against it** — it is evidence that this harness was never going to see it. If
the claim matters, it needs a bench case built at that item count.

### #442 — a 24-brick dab, 15.01 → 13.34 ms (0.889x) — **appears, unsettled**

The closest figures in shape are the dab group and `locality`:

| figure | A median | B median | B/A | |
|---|---:|---:|---:|---|
| `dab.median` | 3.093 ms | 3.443 ms | 1.113x | overlapping |
| `dab.p95` | 4.125 ms | 4.566 ms | 1.107x | overlapping |
| `locality.dab_ms` | 3.361 ms | 3.554 ms | 1.057x | overlapping |
| `locality.dab_ms_10x` | 5.109 ms | 4.607 ms | **0.902x** | overlapping |

`locality.dab_ms_10x` — the dab on the scene ten times the area, the largest dab
here and the closest to a 24-brick one — moved **0.902x**, which is the right
direction and almost exactly the claimed size (0.889x). That is suggestive and
it is not a confirmation: its ranges overlap, and the three smaller dab figures
moved the *other* way by a similar amount. **On this data #442 is neither shown
nor refuted.** The counts confirm the work itself is unchanged — `keys_remeshed`
is 12 on both sides, `surface_bricks` 1,049 on both, `key_ratio` 0.750 on both
— so any difference is cost per brick, which is what #442 claims to change.

### #375 — a surface gesture stops allocating per warp — **does not appear**

Every gesture figure is flat and none is separable from noise:

| figure | A median | B median | B/A |
|---|---:|---:|---:|
| `brush.mesh.mover.mean` | 18.40 ms | 18.27 ms | 0.993x |
| `brush.sdf.mover.mean` | 590.1 ms | 592.2 ms | 1.004x |
| `brush.sdf.movertopologico.mean` | 662.5 ms | 660.9 ms | 0.998x |
| `brush.voxel.mover.ms` | 37.28 ms | 36.85 ms | 0.988x |
| `brush.mesh.nudge.mean` | 10.48 ms | 10.43 ms | 0.996x |
| `brush.voxel.nudge.mean` | 36.18 ms | 35.90 ms | 0.992x |

An allocation per reached item, freed moments later, is the kind of cost a
general-purpose allocator absorbs almost entirely. **The change is very likely
correct and simply too small to see at this scale** — which is a different
statement from "it did not help", and this report makes the first one. What is
visible is on the spread rather than the median: `brush.mesh.mover.mean` ranges
18.25–72.67 ms on A and 17.99–20.08 ms on B, which belongs with the stability
finding above.

### Deferred normals — **wired, measured, and not currently a win**

This is B-only machinery, so there is no ratio against A; it is a comparison
inside B between two arms of the same seam:

| | median | range |
|---|---:|---|
| `normals.direct.mean` | 4.083 ms | 3.924–4.202 |
| `normals.deferred.mean` | 4.428 ms | 4.216–4.546 |
| `normals.deferred_ratio` | **1.080** | 1.066–1.150 |

**Deferring costs about 8% more than it saves**, consistently, in all five runs.
That independently reproduces the finding the bench phase recorded at ~14% on a
different fixture shape, and it means the upgrade cannot claim this seam as a
win without a figure. The de-duplication is real; the per-stamp bookkeeping to
achieve it currently costs more than the duplicate work did.

---

## What the new machinery costs — the 23 B-only figures

These have no counterpart on A and are reported as absolute measurements. A
ratio against a pin that could not run them would be two experiments rather than
two readings of one.

| figure | unit | median | range across five runs |
|---|---|---:|---|
| `maintenance.drain.mean` | ms | 0.0011 | 0.0010–0.0012 |
| `maintenance.drain.p95` | ms | 0.0012 | 0.0009–0.0014 |
| `maintenance.idle.mean` | ms | 0.0001 | 0.0001–0.0001 |
| `maintenance.idle.p95` | ms | 0.0001 | 0.0001–0.0001 |
| `multires.add_level.ms` | ms | 38.1 | 37.8–41.7 |
| `multires.bake_to_base.ms` | ms | 0.692 | 0.651–0.779 |
| `multires.compose.mean` | ms | 19.4 | 19.1–21.7 |
| `multires.compose.p95` | ms | 19.9 | 19.2–22.6 |
| `multires.drop_caches.ms` | ms | 109.5 | 109.2–117.8 |
| `multires.from_mesh.ms` | ms | 15.1 | 14.1–15.8 |
| `multires.merge_down.ms` | ms | 0.695 | 0.683–0.783 |
| `multires.pass_stroke.mean` | ms | 17.2 | 16.0–17.6 |
| `multires.pass_stroke.p95` | ms | 13.9 | 12.1–14.0 |
| `multires.reorder.mean` | ms | 0.036 | 0.033–0.041 |
| `multires.reorder.p95` | ms | 0.028 | 0.025–0.033 |
| `multires.serialize.ms` | ms | 2.13 | 1.98–2.16 |
| `multires.stamp.mean` | ms | 12.3 | 11.8–13.3 |
| `multires.stamp.p95` | ms | 13.4 | 12.0–14.0 |
| `normals.deferred.mean` | ms | 4.43 | 4.22–4.55 |
| `normals.deferred.p95` | ms | 4.63 | 4.38–4.79 |
| `normals.deferred_ratio` | x | 1.08 | 1.07–1.15 |
| `normals.direct.mean` | ms | 4.08 | 3.92–4.20 |
| `normals.direct.p95` | ms | 4.23 | 4.11–4.48 |

**Reading these.** The hierarchy tier is the expensive newcomer and none of it
is on a per-frame path: building one from a mesh layer is 15.1 ms, adding a
level is 38.1 ms, and a stamp at 12.3 ms sits between the median mesh brush
(9.2 ms) and the median voxel brush (15.4 ms) — a sculptor pays hierarchy prices
for hierarchy work and mesh prices for mesh work. `multires.drop_caches.ms` at 109.5 ms is the largest
figure in the group and is the recovery path, not the sculpting path.
`multires.serialize.ms` at 2.13 ms is the cost the engine phase built the undo
side-car on, and it is cheap enough that the "history holds the hierarchy's own
serialized bytes" decision looks affordable from here.

**The maintenance queue is free.** `maintenance.drain` is 1.1 µs and
`maintenance.idle` 0.1 µs, stable to the last digit across five runs. Reading
the sculptor's quality per gesture rather than per segment costs nothing
measurable, which is what the bench phase concluded and what these five runs
confirm.

### What B measured and A did not, on purpose

B skips 15 `brush.multires.*` figures and `convert.multires_to_mesh` with *no
reference member for this representation*. That is the deliberate
`Scene::for_representation(Multires) = None` decision from the model phase:
adding a scene member would change `conditions.scenes`, which is the first thing
`compare::unlike` refuses on, and would stop every committed baseline comparing.
The skips are declared rather than silent, which is the behaviour the harness
was built to have.

**One skip is not in that category and should be looked at.**
`convert.mesh_to_multires` skips with **"the engine refused the edit"** — not a
missing fixture but a refusal from the engine, on all five B runs. Given that
`multires.from_mesh.ms` measures 15.1 ms successfully in the same run, the
hierarchy can plainly be built; something about the *conversion* path's fixture
is being refused. It is out of scope for a measurement report, but it is a
loose end this campaign surfaced and it is written down here rather than left in
a log.

`brush.sdf.trim` skips on **both** sides with "no gesture this harness can
synthesise", unchanged across the pin.

Two figures, `subtool.activate.sdf.mean` and `subtool.activate.sdf.p95`, read
exactly **0.0 on both sides in all ten runs**. They are shape-matched and carry
no information; they are in the table with the ratio shown as "zero both sides"
rather than as a division by zero.

---

## The baseline was not re-recorded

`benchmarks/baseline-linux-x86_64.json` is untouched, per the justfile's own
warning that recording over it is how an A/B run silently destroys the
comparison it was performing.

**It should probably move, and that is a human's call, not this report's.** The
committed baseline was recorded against **engine 0.52.2** with no revision
field, at a load of 0.128 per core. That is many pins ago; every comparison
against it now folds several engine upgrades into whatever is being tested, and
the harness says so out loud in its `across_engines` note. The arguments for
moving it are that it is stale and that it predates both the `spread` and
`revision` sections, so it cannot participate in the within-spread reporting the
bench phase added. The argument against is the one the justfile makes: a
re-record hides whatever regressed since the last one, and **there is currently
an unexplained regression on live intersecting booleans that a re-record would
bury**.

The recommendation is therefore: **resolve `object.drag_frame_intersect` first,
then re-record**, on a genuinely quiet machine, and keep this file as the record
of what the pin move cost.

---

## Summary

- **The pin move is free.** 178 shape-matched figures, median **0.9998x**, 128
  of them within ±5% of parity. This independently reproduces the release's own
  0.9949x from entirely different hardware and a different harness.
- **One regression is real and unexplained:** live *intersecting* booleans cost
  **1.166x** what they did, with the subtracting arm of the same fixture flat.
  This runs directly against claim #319, on a fixture that is not one of #319's
  stated exclusions.
- **One improvement is real and is not in the release notes:** mesh brush
  timings became repeatable. A's spread on `brush.mesh.planar.ms` is 2.97x and
  B's is 1.03x, and interleaving rules out machine drift as the cause.
- **Three of the four claims checked did not appear:** #441 has no instrument
  here, #442 is suggested by one figure and unsettled, #375 is below this
  harness's resolution. Only the deferred-normals seam could be measured
  cleanly, and it **costs 8% more than it saves**.
- **171 of 178 figures could not be separated from noise**, and are reported as
  that rather than as small wins and small losses.

The load caveat stands over all of it: the box was shared, seven of ten runs sat
in the harness's warn band, and the honest next step for anything marginal here
is repeats on a quiet machine.

---

## Appendix: all 178 shape-matched figures

Medians and ranges are across five runs per side. The last column says whether
the two sides' five-run ranges are completely disjoint; `no` means the spread
covers the ratio and the figure has told us nothing, however far from 1.000x it
reads.

| figure | unit | A median | A range | B median | B range | B/A | beyond spread? |
|---|---|---:|---|---:|---|---:|---|
| `authoring.armature.ms` | ms | 76.2 | 74.1–81.1 | 77.0 | 75.6–79.7 | 1.011x | no |
| `authoring.curve.ms` | ms | 34.5 | 33.9–35.7 | 35.3 | 33.9–35.7 | 1.021x | no |
| `authoring.skin.ms` | ms | 120.4 | 120.1–134.0 | 127.1 | 121.8–130.3 | 1.055x | no |
| `bake.consolidate.ms` | ms | 4,522 | 4,463–5,400 | 4,572 | 4,470–5,163 | 1.011x | no |
| `bake.export.ms` | ms | 330.9 | 319.8–431.9 | 337.5 | 321.9–1,495 | 1.020x | no |
| `bake.repair_report.ms` | ms | 2.27 | 2.26–3.22 | 2.26 | 2.25–4.33 | 0.994x | no |
| `brush.mesh.argila.mean` | ms | 8.88 | 8.57–9.95 | 9.18 | 8.94–10.2 | 1.034x | no |
| `brush.mesh.argila.p95` | ms | 9.19 | 8.86–10.5 | 9.49 | 9.38–10.6 | 1.033x | no |
| `brush.mesh.borrar.mean` | ms | 8.38 | 8.03–9.17 | 8.24 | 7.99–9.89 | 0.984x | no |
| `brush.mesh.borrar.p95` | ms | 9.03 | 8.16–9.88 | 8.60 | 8.17–10.4 | 0.952x | no |
| `brush.mesh.camada.mean` | ms | 21.3 | 16.8–69.6 | 17.2 | 17.0–17.3 | 0.807x | no |
| `brush.mesh.camada.p95` | ms | 15.0 | 9.42–32.0 | 9.64 | 9.37–10.5 | 0.642x | no |
| `brush.mesh.inflar.mean` | ms | 9.04 | 8.70–25.0 | 8.95 | 8.79–9.50 | 0.990x | no |
| `brush.mesh.inflar.p95` | ms | 9.84 | 8.89–26.1 | 9.18 | 8.94–10.8 | 0.933x | no |
| `brush.mesh.mascara.mean` | ms | 10.9 | 9.09–26.9 | 9.20 | 9.09–9.56 | 0.842x | no |
| `brush.mesh.mascara.p95` | ms | 11.4 | 9.27–42.4 | 9.70 | 9.23–10.5 | 0.849x | no |
| `brush.mesh.mover.mean` | ms | 18.4 | 18.2–72.7 | 18.3 | 18.0–20.1 | 0.993x | no |
| `brush.mesh.mover.p95` | ms | 10.6 | 9.73–48.9 | 9.86 | 9.67–11.6 | 0.933x | no |
| `brush.mesh.nudge.mean` | ms | 10.5 | 10.3–12.2 | 10.4 | 10.2–11.0 | 0.996x | no |
| `brush.mesh.nudge.p95` | ms | 10.7 | 10.5–12.8 | 11.0 | 10.4–12.1 | 1.033x | no |
| `brush.mesh.padrao.mean` | ms | 8.85 | 8.74–23.0 | 8.76 | 8.67–9.03 | 0.990x | no |
| `brush.mesh.padrao.p95` | ms | 9.24 | 9.11–23.8 | 9.12 | 8.79–9.29 | 0.988x | no |
| `brush.mesh.pincar.mean` | ms | 15.7 | 8.74–39.1 | 8.98 | 8.84–40.1 | 0.574x | no |
| `brush.mesh.pincar.p95` | ms | 19.4 | 8.90–60.3 | 9.48 | 9.08–50.7 | 0.489x | no |
| `brush.mesh.pintar.mean` | ms | 16.8 | 16.3–17.3 | 16.7 | 16.3–18.5 | 0.992x | no |
| `brush.mesh.pintar.p95` | ms | 9.53 | 8.77–9.80 | 8.97 | 8.79–10.3 | 0.941x | no |
| `brush.mesh.planar.ms` | ms | 60.4 | 29.1–86.6 | 25.5 | 24.9–25.7 | 0.422x | **faster** |
| `brush.mesh.polir.ms` | ms | 82.0 | 80.6–233.9 | 76.8 | 76.7–80.9 | 0.937x | no |
| `brush.mesh.puxar.mean` | ms | 12.0 | 10.3–35.3 | 10.4 | 10.3–11.1 | 0.872x | no |
| `brush.mesh.puxar.p95` | ms | 12.6 | 10.7–36.4 | 11.3 | 10.6–11.9 | 0.894x | no |
| `brush.mesh.raspar.mean` | ms | 15.3 | 9.07–25.8 | 9.33 | 9.11–9.50 | 0.611x | no |
| `brush.mesh.raspar.p95` | ms | 17.0 | 9.34–27.6 | 9.54 | 9.36–9.82 | 0.560x | no |
| `brush.mesh.relaxar.ms` | ms | 48.0 | 46.4–129.3 | 41.9 | 41.6–42.5 | 0.872x | **faster** |
| `brush.mesh.suavizar.ms` | ms | 45.9 | 44.9–174.4 | 42.2 | 41.9–44.2 | 0.918x | **faster** |
| `brush.mesh.vinco.mean` | ms | 8.81 | 8.73–9.91 | 8.98 | 8.76–10.5 | 1.019x | no |
| `brush.mesh.vinco.p95` | ms | 9.19 | 8.92–10.7 | 9.15 | 9.13–10.8 | 0.995x | no |
| `brush.sdf.argila.mean` | ms | 11.3 | 10.7–11.7 | 11.4 | 11.1–12.6 | 1.007x | no |
| `brush.sdf.argila.p95` | ms | 15.6 | 15.1–18.4 | 17.3 | 16.1–17.7 | 1.104x | no |
| `brush.sdf.camada.mean` | ms | 10.9 | 10.6–11.5 | 11.1 | 10.8–11.3 | 1.022x | no |
| `brush.sdf.camada.p95` | ms | 15.4 | 15.2–17.3 | 16.6 | 15.1–18.8 | 1.076x | no |
| `brush.sdf.inflar.mean` | ms | 11.6 | 11.0–16.7 | 11.4 | 11.1–11.6 | 0.985x | no |
| `brush.sdf.inflar.p95` | ms | 16.2 | 14.6–29.0 | 15.0 | 14.6–16.9 | 0.925x | no |
| `brush.sdf.mascara.mean` | ms | 0.237 | 0.233–0.253 | 0.235 | 0.234–0.257 | 0.991x | no |
| `brush.sdf.mascara.p95` | ms | 0.265 | 0.253–0.278 | 0.257 | 0.253–0.278 | 0.969x | no |
| `brush.sdf.mover.mean` | ms | 590.1 | 582.5–620.2 | 592.2 | 589.6–595.1 | 1.004x | no |
| `brush.sdf.mover.p95` | ms | 1,103 | 1,092–1,252 | 1,112 | 1,101–1,137 | 1.008x | no |
| `brush.sdf.movertopologico.mean` | ms | 662.5 | 657.3–720.7 | 660.9 | 655.9–673.6 | 0.998x | no |
| `brush.sdf.movertopologico.p95` | ms | 1,148 | 1,126–1,272 | 1,144 | 1,112–1,156 | 0.997x | no |
| `brush.sdf.padrao.mean` | ms | 10.5 | 10.3–14.4 | 10.9 | 10.6–12.0 | 1.044x | no |
| `brush.sdf.padrao.p95` | ms | 16.9 | 14.8–29.1 | 15.6 | 15.1–20.2 | 0.925x | no |
| `brush.sdf.planar.ms` | ms | 172.9 | 171.4–185.3 | 173.9 | 172.6–180.3 | 1.006x | no |
| `brush.sdf.polir.ms` | ms | 175.7 | 174.8–181.3 | 174.3 | 173.6–175.1 | 0.992x | no |
| `brush.sdf.puxar.mean` | ms | 56.4 | 55.6–59.3 | 58.2 | 56.5–59.5 | 1.031x | no |
| `brush.sdf.puxar.p95` | ms | 61.0 | 60.3–68.8 | 66.1 | 60.2–72.9 | 1.085x | no |
| `brush.sdf.relaxar.ms` | ms | 175.2 | 174.0–180.0 | 174.0 | 173.3–179.6 | 0.993x | no |
| `brush.sdf.suavizar.ms` | ms | 171.8 | 169.0–234.2 | 170.9 | 170.4–171.7 | 0.995x | no |
| `brush.sdf.vinco.mean` | ms | 18.0 | 17.3–18.6 | 17.7 | 17.4–18.3 | 0.982x | no |
| `brush.sdf.vinco.p95` | ms | 14.6 | 13.9–16.8 | 14.7 | 14.5–14.8 | 1.006x | no |
| `brush.voxel.apagar.mean` | ms | 16.4 | 16.1–37.6 | 16.3 | 16.0–16.6 | 0.995x | no |
| `brush.voxel.apagar.p95` | ms | 31.2 | 29.8–77.2 | 30.8 | 29.6–31.6 | 0.986x | no |
| `brush.voxel.camada.mean` | ms | 14.1 | 14.0–28.6 | 14.3 | 14.1–14.4 | 1.010x | no |
| `brush.voxel.camada.p95` | ms | 30.6 | 30.3–65.4 | 30.6 | 30.5–31.0 | 1.001x | no |
| `brush.voxel.inflar.mean` | ms | 23.7 | 23.2–25.8 | 23.4 | 23.3–24.4 | 0.987x | no |
| `brush.voxel.inflar.p95` | ms | 31.9 | 30.4–34.5 | 30.7 | 30.5–34.3 | 0.964x | no |
| `brush.voxel.mascara.mean` | ms | 1.09 | 1.07–2.00 | 1.07 | 1.06–1.15 | 0.985x | no |
| `brush.voxel.mascara.p95` | ms | 1.18 | 1.13–2.17 | 1.13 | 1.12–1.20 | 0.958x | no |
| `brush.voxel.mover.ms` | ms | 37.3 | 36.2–41.5 | 36.8 | 36.5–37.6 | 0.988x | no |
| `brush.voxel.nudge.mean` | ms | 36.2 | 35.7–71.7 | 35.9 | 35.8–37.3 | 0.992x | no |
| `brush.voxel.nudge.p95` | ms | 39.1 | 37.4–75.4 | 37.1 | 36.4–38.6 | 0.948x | no |
| `brush.voxel.padrao.mean` | ms | 14.5 | 14.0–16.1 | 14.2 | 14.0–14.7 | 0.975x | no |
| `brush.voxel.padrao.p95` | ms | 31.3 | 30.3–35.1 | 30.5 | 30.5–31.5 | 0.977x | no |
| `brush.voxel.pincar.mean` | ms | 30.5 | 30.2–32.1 | 30.7 | 30.2–31.7 | 1.009x | no |
| `brush.voxel.pincar.p95` | ms | 31.7 | 30.4–34.5 | 32.3 | 30.5–33.3 | 1.018x | no |
| `brush.voxel.pintar.mean` | ms | 0.342 | 0.340–0.618 | 0.342 | 0.338–0.377 | 0.999x | no |
| `brush.voxel.pintar.p95` | ms | 0.353 | 0.348–0.669 | 0.352 | 0.346–0.388 | 0.999x | no |
| `brush.voxel.planar.ms` | ms | 36.2 | 35.6–151.1 | 36.2 | 35.8–36.3 | 1.000x | no |
| `brush.voxel.preencher.mean` | ms | 14.2 | 14.2–41.7 | 14.5 | 14.3–15.2 | 1.019x | no |
| `brush.voxel.preencher.p95` | ms | 30.2 | 30.2–115.3 | 31.0 | 30.1–33.8 | 1.026x | no |
| `brush.voxel.raspar.mean` | ms | 28.4 | 27.4–63.1 | 28.0 | 27.9–29.2 | 0.986x | no |
| `brush.voxel.raspar.p95` | ms | 33.4 | 30.4–112.5 | 33.2 | 32.2–33.7 | 0.995x | no |
| `brush.voxel.suavizar.ms` | ms | 36.3 | 35.9–37.4 | 36.4 | 35.9–36.6 | 1.004x | no |
| `convert.mesh_to_sdf.ms` | ms | 7,982 | 7,897–8,368 | 7,980 | 7,811–8,892 | 1.000x | no |
| `convert.mesh_to_voxel.ms` | ms | 1,450 | 1,409–1,493 | 1,413 | 1,397–5,113 | 0.975x | no |
| `convert.sdf_to_mesh.ms` | ms | 328.5 | 322.2–1,243 | 331.7 | 328.5–348.5 | 1.010x | no |
| `convert.sdf_to_voxel.ms` | ms | 453.2 | 449.7–470.2 | 452.8 | 449.0–472.5 | 0.999x | no |
| `convert.voxel_to_mesh.ms` | ms | 14.1 | 14.0–34.8 | 14.1 | 13.7–14.6 | 1.006x | no |
| `convert.voxel_to_sdf.ms` | ms | 16,437 | 15,549–17,268 | 16,514 | 15,828–27,448 | 1.005x | no |
| `dab.median` | ms | 3.09 | 3.06–3.61 | 3.44 | 3.11–3.55 | 1.113x | no |
| `dab.p95` | ms | 4.12 | 3.71–12.4 | 4.57 | 3.88–5.39 | 1.107x | no |
| `frame.median` | ms | 0.613 | 0.604–0.730 | 0.610 | 0.602–0.617 | 0.995x | no |
| `frame.p95` | ms | 0.954 | 0.943–3.70 | 0.954 | 0.950–0.961 | 1.000x | no |
| `history.edit.mean` | ms | 3.58 | 3.38–3.69 | 3.91 | 3.39–4.07 | 1.093x | no |
| `history.edit.p95` | ms | 4.26 | 3.88–4.85 | 4.96 | 3.71–5.42 | 1.164x | no |
| `history.redo.mean` | ms | 4.39 | 4.05–4.58 | 4.67 | 4.08–5.08 | 1.063x | no |
| `history.redo.p95` | ms | 5.24 | 4.41–5.84 | 5.85 | 4.34–6.30 | 1.117x | no |
| `history.undo.mean` | ms | 4.29 | 4.16–4.52 | 4.71 | 4.07–4.86 | 1.098x | no |
| `history.undo.p95` | ms | 5.05 | 4.93–5.22 | 5.77 | 4.28–5.92 | 1.142x | no |
| `history.undo_ratio` | x | 1.23 | 1.16–1.26 | 1.19 | 1.19–1.24 | 0.973x | no |
| `locality.dab_ms` | ms | 3.36 | 3.15–7.68 | 3.55 | 3.26–4.70 | 1.057x | no |
| `locality.dab_ms_10x` | ms | 5.11 | 4.69–6.07 | 4.61 | 4.58–4.94 | 0.902x | no |
| `locality.key_ratio` | x | 0.750 | 0.750–0.750 | 0.750 | 0.750–0.750 | 1.000x | no |
| `locality.keys_remeshed` |  | 12.0 | 12.0–12.0 | 12.0 | 12.0–12.0 | 1.000x | no |
| `locality.keys_remeshed_10x` |  | 9.00 | 9.00–9.00 | 9.00 | 9.00–9.00 | 1.000x | no |
| `locality.surface_bricks` |  | 1,049 | 1,049–1,049 | 1,049 | 1,049–1,049 | 1.000x | no |
| `locality.surface_bricks_10x` |  | 10,185 | 10,185–10,185 | 10,185 | 10,185–10,185 | 1.000x | no |
| `mask.gated.mean` | ms | 3.09 | 2.92–3.16 | 3.27 | 3.01–15.8 | 1.058x | no |
| `mask.gated.p95` | ms | 4.17 | 3.95–4.48 | 4.70 | 4.10–23.1 | 1.128x | no |
| `mask.gated_ratio` | x | 0.877 | 0.709–0.958 | 0.830 | 0.770–0.908 | 0.947x | no |
| `mask.outline.mean` | ms | 576.7 | 571.6–579.3 | 578.7 | 573.8–879.8 | 1.003x | no |
| `mask.outline.p95` | ms | 590.7 | 581.1–593.5 | 586.9 | 584.3–1,320 | 0.994x | no |
| `mask.ungated.mean` | ms | 3.37 | 3.29–4.12 | 3.78 | 3.56–20.4 | 1.122x | no |
| `mask.ungated.p95` | ms | 3.74 | 3.62–5.14 | 4.97 | 4.31–29.0 | 1.326x | no |
| `memory.baseline` | MB | 1.02 | 1.02–1.02 | 1.02 | 1.02–1.02 | 1.000x | no |
| `memory.budget` | MB | 512.0 | 512.0–512.0 | 512.0 | 512.0–512.0 | 1.000x | no |
| `memory.drift` | x | 1.00 | 1.00–1.00 | 1.00 | 1.00–1.00 | 1.000x | no |
| `memory.peak` | MB | 1.02 | 1.02–1.02 | 1.02 | 1.02–1.02 | 1.000x | no |
| `msaa.1x.frame.median` | ms | 0.321 | 0.262–0.345 | 0.263 | 0.262–0.320 | 0.821x | no |
| `msaa.2x.frame.median` | ms | 0.250 | 0.247–0.309 | 0.247 | 0.245–0.312 | 0.991x | no |
| `msaa.4x.frame.median` | ms | 0.342 | 0.338–0.415 | 0.339 | 0.336–0.400 | 0.993x | no |
| `object.drag_frame.mean` | ms | 25.5 | 25.0–26.2 | 25.8 | 24.9–32.4 | 1.013x | no |
| `object.drag_frame.p95` | ms | 23.4 | 23.1–24.3 | 23.6 | 22.9–33.7 | 1.008x | no |
| `object.drag_frame_intersect.mean` | ms | 57.3 | 56.2–58.8 | 66.8 | 65.4–97.0 | 1.166x | **slower** |
| `object.drag_frame_intersect.p95` | ms | 58.9 | 55.1–64.9 | 69.3 | 65.2–76.3 | 1.176x | **slower** |
| `object.pick.ms` | ms | 0.100 | 0.096–0.101 | 0.113 | 0.102–0.136 | 1.133x | **slower** |
| `object.place.ms` | ms | 15.9 | 15.4–16.3 | 16.2 | 15.7–19.0 | 1.021x | no |
| `object.re_op.ms` | ms | 14.0 | 13.2–15.1 | 14.6 | 13.5–16.9 | 1.049x | no |
| `object.re_shape.ms` | ms | 15.7 | 15.3–16.8 | 15.9 | 15.6–18.8 | 1.008x | no |
| `object.remove.ms` | ms | 5.32 | 5.02–5.71 | 5.17 | 5.01–7.09 | 0.970x | no |
| `op.mesh.lattice.ms` | ms | 36.7 | 34.9–38.5 | 36.2 | 35.8–42.2 | 0.986x | no |
| `op.mesh.taper.ms` | ms | 22.6 | 21.8–23.1 | 22.0 | 21.8–23.3 | 0.975x | no |
| `op.mesh.twist.ms` | ms | 22.8 | 22.1–25.6 | 22.6 | 22.5–25.1 | 0.991x | no |
| `op.voxel.close_holes.ms` | ms | 19.6 | 19.4–20.6 | 20.6 | 19.7–23.4 | 1.053x | no |
| `op.voxel.fill_voids.ms` | ms | 19.7 | 19.6–20.5 | 20.2 | 19.8–22.9 | 1.022x | no |
| `op.voxel.refine.ms` | ms | 6.39 | 6.28–6.43 | 6.56 | 6.33–7.37 | 1.027x | no |
| `render.1080p.ao_off.frame.median` | ms | 0.164 | 0.163–0.165 | 0.165 | 0.163–0.167 | 1.006x | no |
| `render.1080p.ao_off.frame.p95` | ms | 0.181 | 0.179–0.182 | 0.180 | 0.177–0.182 | 0.997x | no |
| `render.1080p.draws` |  | 4.00 | 4.00–4.00 | 4.00 | 4.00–4.00 | 1.000x | no |
| `render.1080p.frame.median` | ms | 0.337 | 0.333–0.371 | 0.338 | 0.336–0.341 | 1.004x | no |
| `render.1080p.frame.p95` | ms | 0.344 | 0.338–0.627 | 0.356 | 0.343–0.402 | 1.033x | no |
| `render.1080p.gpu.ao` | ms | 0.029 | 0.028–0.030 | 0.028 | 0.028–0.029 | 0.969x | no |
| `render.1080p.gpu.ao_composite` | ms | 0.045 | 0.045–0.046 | 0.045 | 0.045–0.046 | 0.996x | no |
| `render.1080p.gpu.depth_reduce` | ms | 0.074 | 0.073–0.075 | 0.072 | 0.071–0.073 | 0.970x | **faster** |
| `render.1080p.gpu.scene` | ms | 0.107 | 0.106–0.109 | 0.106 | 0.104–0.107 | 0.995x | no |
| `render.1080p.triangles` |  | 395,392 | 395,392–395,392 | 395,392 | 395,392–395,392 | 1.000x | no |
| `render.1440p.ao_off.frame.median` | ms | 0.211 | 0.205–0.233 | 0.212 | 0.210–0.253 | 1.007x | no |
| `render.1440p.ao_off.frame.p95` | ms | 0.213 | 0.206–0.622 | 0.218 | 0.214–0.257 | 1.026x | no |
| `render.1440p.draws` |  | 4.00 | 4.00–4.00 | 4.00 | 4.00–4.00 | 1.000x | no |
| `render.1440p.frame.median` | ms | 0.481 | 0.480–0.528 | 0.480 | 0.478–0.492 | 1.000x | no |
| `render.1440p.frame.p95` | ms | 0.486 | 0.483–0.631 | 0.486 | 0.481–0.500 | 1.000x | no |
| `render.1440p.gpu.ao` | ms | 0.045 | 0.044–0.045 | 0.045 | 0.043–0.046 | 0.993x | no |
| `render.1440p.gpu.ao_composite` | ms | 0.081 | 0.080–0.082 | 0.080 | 0.080–0.081 | 0.994x | no |
| `render.1440p.gpu.depth_reduce` | ms | 0.117 | 0.115–0.120 | 0.117 | 0.116–0.121 | 1.008x | no |
| `render.1440p.gpu.scene` | ms | 0.152 | 0.148–0.155 | 0.151 | 0.149–0.157 | 0.994x | no |
| `render.1440p.triangles` |  | 395,392 | 395,392–395,392 | 395,392 | 395,392–395,392 | 1.000x | no |
| `render.2160p.ao_off.frame.median` | ms | 0.396 | 0.360–0.447 | 0.363 | 0.362–0.366 | 0.915x | no |
| `render.2160p.ao_off.frame.p95` | ms | 0.419 | 0.363–1.30 | 0.373 | 0.363–0.375 | 0.889x | no |
| `render.2160p.draws` |  | 4.00 | 4.00–4.00 | 4.00 | 4.00–4.00 | 1.000x | no |
| `render.2160p.frame.median` | ms | 0.929 | 0.926–1.01 | 0.930 | 0.919–1.02 | 1.001x | no |
| `render.2160p.frame.p95` | ms | 0.934 | 0.933–1.15 | 0.935 | 0.929–1.05 | 1.000x | no |
| `render.2160p.gpu.ao` | ms | 0.090 | 0.090–0.094 | 0.090 | 0.088–0.093 | 0.997x | no |
| `render.2160p.gpu.ao_composite` | ms | 0.180 | 0.180–0.181 | 0.180 | 0.179–0.181 | 0.999x | no |
| `render.2160p.gpu.depth_reduce` | ms | 0.260 | 0.257–0.262 | 0.260 | 0.259–0.262 | 1.001x | no |
| `render.2160p.gpu.scene` | ms | 0.310 | 0.306–0.312 | 0.309 | 0.304–0.313 | 0.997x | no |
| `render.2160p.triangles` |  | 395,392 | 395,392–395,392 | 395,392 | 395,392–395,392 | 1.000x | no |
| `startup.backend_discovery` | ms | 0.0061 | 0.0056–0.0075 | 0.0057 | 0.0052–0.0066 | 0.934x | no |
| `startup.to_first_document` | ms | 12.4 | 12.0–217.9 | 12.3 | 12.0–12.5 | 0.990x | no |
| `subtool.activate.mesh.mean` | ms | 0.0002 | 0.0001–0.0002 | 0.0002 | 0.0001–0.0002 | 1.000x | no |
| `subtool.activate.mesh.p95` | ms | 0.0001 | 0.0001–0.0002 | 0.0001 | 0.0001–0.0002 | 1.000x | no |
| `subtool.activate.sdf.mean` | ms | 0 | 0–0 | 0 | 0–0 | — | zero both sides |
| `subtool.activate.sdf.p95` | ms | 0 | 0–0 | 0 | 0–0 | — | zero both sides |
| `subtool.boolean.ms` | ms | 11,355 | 10,925–11,920 | 11,524 | 11,264–12,294 | 1.015x | no |
| `subtool.copy.ms` | ms | 5,440 | 5,215–6,061 | 5,461 | 5,407–5,701 | 1.004x | no |
| `subtool.solo.mean` | ms | 14.3 | 13.8–14.9 | 14.7 | 13.7–16.3 | 1.028x | no |
| `subtool.solo.p95` | ms | 14.9 | 14.4–15.9 | 16.4 | 14.9–18.3 | 1.101x | no |
| `subtool.solo_undo.ms` | ms | 181.4 | 170.9–196.4 | 173.3 | 171.4–210.8 | 0.955x | no |
| `tape.dab_after_96_edits` | ms | 3.38 | 3.23–3.58 | 3.29 | 3.25–3.37 | 0.972x | no |
| `tape.dab_on_fresh` | ms | 2.02 | 1.91–7.88 | 2.00 | 1.92–2.16 | 0.993x | no |
| `tape.growth` | x | 1.67 | 0.454–1.77 | 1.64 | 1.56–1.69 | 0.979x | no |
