## Context

See proposal.md — Why. What matters for the approach:

- `crates/clayspace-app/src/bin/bench.rs` is one 671-line file: six
  `measure_*` functions filling a `BTreeMap<String, Figure>`, a hand-rolled
  JSON writer, and a `compare` that reads the baseline with `str::find`. Each
  measurement bails out with `let ... else { return; }` when a precondition is
  missing, which produces no figure and no word about why.
- `Figure` already carries `budget`, `tolerance` and `noise_floor`, and
  `regressed_against` already handles the small-number case. That model is
  sound and stays.
- `compare` iterates the *current* figures and looks each one up in the
  baseline text. A figure the baseline has and this run does not is skipped
  without comment — the hole the spec's new requirement closes.
- `Scene` builds SDF documents only: `add_starting_sphere` plus eight bands of
  strokes, at one of two radii. There is no voxel or mesh reference form.
- `ToolKind::for_representation` already answers which tools exist on a
  representation, from the same table the shelf uses.
- `LayerOperation` has no `ALL`; `DeformSettings` and `ConversionSettings`
  reach the document through the ViewModel's `RunDeform` / `RunConversion`.
- The existing baselines are 20 figures each. The suite proposed here is
  roughly 90.

## Goals / Non-Goals

**Goals:**

- Coverage that cannot silently rot: adding a tool or a layer operation makes
  the benchmark fail to build or report the gap, rather than quietly measuring
  one fewer thing.
- One record. A figure lives in the baseline file, not in a doc comment.
- A run that stays affordable in CI, and says what it cost.
- Per-function cognitive complexity inside the frontend band, which a
  measurement file naturally violates if it keeps growing as one module.

**Non-Goals:**

- New budgets. Every figure added here is reported and compared, never
  asserted. The specification's five budgets are unchanged.
- Measuring the interface. Nothing here opens a window; the 16 ms
  interface-thread rule stays with `instrumentation.rs`.
- Replacing the profiling tests. `stroke_budget`, `dab_profile`,
  `mesh_scaling` and `undo_cost` answer *where the time goes*, which a single
  figure cannot; they keep their assertions and lose only their role as the
  record.
- Cross-machine comparison. The gate stays same-machine, same-suite.

## Decisions

### The suite is derived, not listed

The brush group iterates `Representation::ALL × ToolKind::for_representation`,
so a tool added to the shelf is a tool measured, with no second list to update.
Layer operations get a `LayerOperation::ALL` built from an exhaustive `match`
on a variant — adding a variant then fails to compile, which is the strongest
form of "the absence of a figure is reported" the language offers.

Alternative considered: a hand-written table of what to measure, with a test
asserting it covers `ToolKind::ALL`. Rejected — that is a second place to
forget, and the test would pass the day the table went stale for
`LayerOperation`, which has no `ALL` to check against.

Where a derived pair cannot actually be measured — a tool that needs a gesture
the harness does not synthesise — the group records a *skip with a reason*
rather than dropping the figure. See below.

### Figures name their operation; existing names do not move

`<group>.<representation>.<operation>.<statistic>`, lowercase, e.g.
`brush.sdf.padrao.median`, `brush.voxel.apagar.p95`, `op.mesh.taper.ms`,
`convert.sdf_to_voxel.ms`, `mask.gated_ratio`, `history.undo_ratio`.

The twenty figures already in the baselines keep their current names
(`dab.median`, `locality.key_ratio`, `tape.growth`, …) even though they do not
fit the scheme. Renaming them would make every one of them read as *missing*
on the first comparison and would break the continuity with the recorded
history in `stroke_budget.rs`. They are the five specified budgets; the scheme
governs what is being added.

### One record per figure, declared in one table

Each figure declares its repetition count, its tolerance and whether it is
`Repeatable` (dabs onto a document that may accumulate) or `OneShot` (a
conversion, a bake, an export — measured on a document rebuilt for it). The
table lives at the top of one module so the cost/noise trade-off is read in one
place rather than inferred from twelve call sites.

- Repeatable: 12 samples, reported as a **mean** and a p95, at tolerance 1.5
  and 2.0.

  Both of those departures were forced by running the gate against its own
  baseline on an unchanged tree, which failed twice before it passed.

  The mean, because a stroke's segments are not repeated measurements of one
  quantity: each dab lands on more surface and a longer tape than the one
  before, so the samples rise across the gesture. `brush.sdf.padrao` measured
  4.7, 5.6, 5.6, 8.2, 8.2, 8.2, 8.7, 12.5, 14.8, 15.0, 16.3, 18.9, 21.7 ms —
  there is a gap in the middle of that, the median falls in it, and which side
  it lands on is noise. Three consecutive runs of unchanged code reported
  medians of 8.68, 11.36 and 8.01; the means of those same sample sets were
  11.41, 11.66 and 10.70. A median is the robust statistic when the samples
  are one quantity measured repeatedly; what a sculptor pays for a gesture is
  the whole of it.

  The wider tolerance on the tail, because a 95th percentile of twelve samples
  is the second largest of them, so one sample delayed by an allocator or
  another process moves it and moves nothing else. One mesh brush's p95 ranged
  over 22.3 to 29.0 ms across four unchanged runs while its central figure
  stayed inside 10 %.
- OneShot: 3 samples with a rebuild between, reported as the median, tolerance
  2.0. A rebuild is excluded from the timing.

Alternative considered: one sample for every one-shot operation, to keep the
run short. Rejected — a single sample against a 1.5 tolerance produces false
regressions, and a tolerance wide enough to absorb it stops detecting anything.
Three-and-widened is the compromise, and the group's own wall clock makes the
cost of that choice visible rather than assumed.

### Skipped is a third state, distinct from missing

A measurement returns `Ok(figures)` or `Err(Skipped { reason })`. The report
prints a skipped section; the JSON records skips alongside figures; `compare`
classifies each baseline figure as *present*, *skipped* (reported, does not
fail) or *missing* (fails).

This is what turns `let ... else { return; }` from a silent hole into a stated
one, and it is what lets a machine with no headless GPU still run the gate
usefully. The reason strings are fixed, not formatted, so a skip is comparable
across runs.

### The suite's identity is a map, not a string

`Conditions` gains `scenes: BTreeMap<&str, &str>` — member name to revision —
and `scene` is retired. `compare` refuses when the maps differ and names the
member that changed, which is what the modified reference-scene requirement
asks for. A baseline predating the field is refused with "does not state its
scenes", the existing wording for a baseline that cannot be compared.

Alternative considered: keeping `scene` as one suite-wide revision. Rejected —
a suite revision has to be bumped by hand whenever any member changes, and the
one thing this field exists to prevent is exactly the mistake of forgetting to
bump it.

### The JSON stays hand-written

The file has one shape and one writer, and the existing comment gives the
reason: a serialiser in the dependency graph is something the licence audit
carries forever for one file. The reader stops being `str::find` over the raw
text, though — with skips and a nested `scenes` map, positional string
searching is how a subtly wrong comparison gets shipped. A ~120-line
hand-written parser for this one shape, with its own unit tests, replaces it.

Alternative considered: adding `serde_json`. Reasonable, and worth revisiting
if a second consumer appears; not worth reopening the audit question for one
file that has not changed shape in a year.

### `bench.rs` becomes `bench/`

`crates/clayspace-app/src/bin/bench/main.rs` plus `figures.rs` (the `Figure`
model, the record table, skips), `report.rs`, `json.rs` (writer and parser),
`compare.rs`, and `groups/` — `brushes.rs`, `operations.rs`, `deform.rs`,
`convert.rs`, `mask.rs`, `history.rs`, `render.rs`, `memory.rs`, `startup.rs`.

Cargo picks up `src/bin/bench/main.rs` as the same `bench` binary, so
`just bench` and every recorded invocation are unchanged. Splitting is not
cosmetic: the measurement bodies are each a build-arrange-time-record sequence
with early exits, and one file of ninety of them is how a scene gets built for
the wrong group.

### `--only <prefix>` filters; it cannot record

`--only brush.voxel` measures and reports only figures whose name starts with
the prefix. `--only` with `--json` is refused, because a baseline recorded from
a subset reports every omitted figure as missing on the next comparison — which
is now a gate failure, and would be a confusing one.

### Every run warms the machine first

Five seconds of discarded sculpting before anything is timed. Measured on an
RTX 5060, an SDF brush read 11.9 ms starting from an idle card and 7.8 ms when
a previous run had left it boosted — a swing larger than any regression this
gate is meant to detect, decided by what the machine happened to be doing
beforehand. A baseline recorded in one state fails every run taken in the
other.

Not a measurement and not reported as one; it is the same class of precaution
as refusing to compare two machines, which the gate already does.

### Timing is measured the way a sculptor pays for it

A brush figure times `apply_stroke` **plus** `SurfaceGeometry::sync`, as
`dab.*` does — the surface reaching the GPU is what makes an edit visible, and
timing the engine call alone measures the half that was never the problem. A
one-shot operation times the operation and the re-mesh it dirties, on the same
grounds.

## Risks / Trade-offs

- **The suite's wall clock.** ~90 figures, several needing a rebuilt document,
  against a run that is under a minute today → the run reports per-group and
  total duration from the first commit, and groups are added in the task order
  below so the cost is watched as it accrues rather than discovered at the end.
  If the total passes ten minutes, `--only` plus a nightly full run is the
  fallback, and the change states that rather than silently accepting it.
- **One-shot figures are noisy at three samples.** → tolerance 2.0 on them, and
  they are reported without budgets. A doubling is still caught; a 40 % drift
  is not, and that is the stated price.
- **The macOS baseline cannot be recorded from this machine.** → the change
  lands with the Linux baseline re-recorded and the macOS file marked stale in
  its `conditions`. Because the `scenes` map will not match, a macOS run
  refuses to compare and says why, rather than reporting ninety regressions.
  Re-recording it is a task, and the change is not archived until it is done.
- **Every existing figure is invalidated once**, because the suite's identity
  field changes shape. → one re-record per platform, in its own commit, with
  the reason in the message; the figures themselves are unchanged by this work,
  so the new baseline's overlapping twenty should match the old within noise
  and that is checked by hand before committing.
- **A derived brush loop will try tools the harness cannot drive** — the
  region-based four do not decompose into segments, `Mascara` paints rather
  than displaces, `Pintar`/`Borrar` move no vertex. → each is measured by the
  gesture it actually takes, and where the harness genuinely cannot produce one
  it records a skip with a reason, which the gate reports. A skip that is
  really a gap is visible in the report every run.
- **`compare` failing on missing figures will fail the first CI run after any
  measurement is removed on purpose.** → removing a figure requires
  re-recording the baseline, which is already a deliberate act with a stated
  reason. That is the intent, not a side effect.

## Migration Plan

1. The measurement groups land before the baselines move; until then the run
   reports new figures and compares the old twenty, since `compare` skips
   figures the baseline does not have.
2. `Conditions` changing shape is the breaking step. It lands together with the
   Linux re-record, so `just bench-compare` is never broken on Linux for more
   than one commit.
3. macOS is re-recorded on a macOS machine and committed separately. Between
   those two commits, `just bench-compare` on macOS refuses to compare and says
   which scenes differ — loud, and correct.
4. Rollback is the baseline files plus one revert; nothing here changes the
   application.
