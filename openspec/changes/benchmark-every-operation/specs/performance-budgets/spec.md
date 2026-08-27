## MODIFIED Requirements

### Requirement: A reference scene defines what the budgets are measured against
The project SHALL define a reference *suite* — one reference document per
representation the application can sculpt, plus a larger variant of one of them
for locality — and a reference machine configuration for each supported
platform. Every performance budget and every reported figure SHALL be stated
and measured against a named member of that suite. Each member SHALL carry its
own revision, and a change to what a member builds SHALL change that revision.
Budgets SHALL NOT be asserted against an unspecified scene.

This replaces the previous rule naming a single reference document. A voxel
verb has nowhere to land on an SDF document and a mesh brush has nowhere to
land on either, so one document cannot carry the measurements; what the
previous rule was protecting — that a figure is never compared against one
taken elsewhere — is preserved by revisioning each member instead.

#### Scenario: Budgets name their conditions
- **WHEN** a performance budget is reported
- **THEN** it names the reference document it was measured on, the platform,
  the active backend and the viewport resolution it was measured at

#### Scenario: A scene changing shape invalidates its baseline
- **WHEN** a reference suite member is changed and its revision moves
- **THEN** a comparison against a baseline recorded at the previous revision is
  refused and says which member differs, rather than reporting the difference
  as a regression

### Requirement: Performance is measured in CI, not asserted
The project SHALL include a repeatable benchmark, runnable locally and in CI,
reporting figures that can be compared across revisions. Its coverage SHALL
extend to every operation a sculptor can invoke that changes the document or
produces geometry — each brush on each representation it has a verb for, each
layer operation, each deformer, rigging, each conversion, mask painting and
mask-gated editing, undo and redo, consolidation and export — as well as dab
latency, frame time, edit locality, startup and memory.

A figure SHALL be reported without a budget unless the specification states one
for that operation. A figure without a budget is a tracked quantity, compared
against the recorded baseline; it is not a promise to the user.

This replaces the previous rule, which named five measurements. Those five
stand; what has changed is that they are a floor rather than the whole of it.

#### Scenario: Benchmarks are comparable across revisions
- **WHEN** the benchmark runs on two revisions on the same machine
- **THEN** it produces figures for the same measurements under the same
  conditions, suitable for direct comparison

#### Scenario: Every invocable operation has a figure
- **WHEN** an operation that changes the document is added to the application
- **THEN** the benchmark reports a figure for it, and the absence of one is
  reported as uncovered rather than passing silently

#### Scenario: An engine version change is comparable
- **WHEN** the benchmark runs before and after the engine pin moves, on the
  same machine and the same reference suite
- **THEN** the comparison is performed and reports, per figure, the before
  value, the after value and the change, with the engine version of each run
  stated

## ADDED Requirements

### Requirement: A figure that stops being measured is reported
The comparison SHALL report any figure present in the baseline that the current
run did not produce, and SHALL treat it as a failure of the gate in the same
way a regression is. A measurement that has stopped running SHALL NOT be
indistinguishable from one that has not regressed.

Where a figure is legitimately unavailable — no accelerated backend, no
headless GPU, a representation the build cannot construct — the run SHALL say
which figures it skipped and why, and a comparison against a baseline that
contains them SHALL report them as skipped rather than as missing.

#### Scenario: A measurement silently stops running
- **WHEN** a change causes a measurement to return early and produce no figure,
  and the baseline contains that figure
- **THEN** the comparison reports the figure as missing and the gate fails

#### Scenario: A measurement is unavailable on this machine
- **WHEN** a measurement cannot run because the machine offers no headless GPU
- **THEN** the run states which figures were skipped and for what reason, and
  the comparison reports them as skipped without failing the gate

### Requirement: Figures are named so an operation can be found in them
Every figure SHALL be named by the group it belongs to, the representation it
was measured on where more than one is possible, the operation, and the
statistic — so that a figure can be traced to the operation that produced it
without reading the benchmark's source, and so that all figures for one
operation sort together.

#### Scenario: Locating what regressed
- **WHEN** a comparison reports a regression
- **THEN** the figure's name identifies the operation and the representation it
  was measured on

### Requirement: The benchmark can be run for one group at a time
The benchmark SHALL accept a filter selecting a subset of its figures by name,
measuring and reporting only those, so that investigating one operation does
not require running the whole suite. A filtered run SHALL NOT be usable to
record a baseline, since a baseline recorded from a subset would report every
omitted figure as missing on the next comparison.

#### Scenario: Measuring one group
- **WHEN** the benchmark is run with a filter naming one group
- **THEN** only that group's figures are measured and reported

#### Scenario: A filtered run cannot record a baseline
- **WHEN** a filtered run is asked to write a baseline file
- **THEN** it refuses and says that a baseline must be recorded from a complete
  run

### Requirement: The benchmark states what it costs to run
The benchmark SHALL report its own wall-clock duration, in total and per
measurement group, so that the cost of running it in CI is known and a group
that has become disproportionately expensive is visible.

#### Scenario: A group becomes expensive
- **WHEN** the benchmark completes
- **THEN** it reports the time each measurement group took and the total

### Requirement: A figure records the machine load it was measured against
The benchmark SHALL sample the machine's one-minute load average before any
measurement begins, report it, and record it in the baseline file, so that a
figure carries the conditions it was taken under rather than being read as
though the machine were idle. The load SHALL be judged per core, since the same
absolute load means something different on a four-core laptop and a twenty-four
core workstation.

A run measured against other work SHALL NOT be usable to record a baseline
unless the operator overrides it explicitly, because a noisy baseline stays
wrong for every run that later compares against it, while a noisy comparison is
wrong only once.

The threshold is evidence rather than convention: on the reference machine a
concurrent test suite and database, around 0.2 runnable threads per core, moved
the move-brush figure by under 2% across three runs, while an unrelated process
at roughly 0.6 per core moved a single measurement by 25% — several times the
tolerance the gate applies.

#### Scenario: A run states the load it was measured under
- **WHEN** the benchmark starts
- **THEN** it reports the one-minute load, the core count and the load per core
- **AND** says so plainly when the machine is not quiet

#### Scenario: A busy machine cannot record a baseline
- **WHEN** a run measured above the per-core threshold is asked to write a
  baseline file
- **THEN** it refuses, names the load, and writes nothing
- **AND** records the baseline anyway if the operator passes an explicit
  override

#### Scenario: A regression reported from a busy machine is caveated
- **WHEN** a comparison reports a regression and the run was not measured on a
  quiet machine
- **THEN** the load is stated alongside the regressions, so that red is not
  acted on before it is reproduced

#### Scenario: A baseline recorded on a busy machine is suspected
- **WHEN** a quiet run reports a regression against a baseline whose recorded
  load was not quiet
- **THEN** the comparison says the baseline may be the wrong half of the
  comparison

#### Scenario: A baseline predating the load record is not assumed quiet
- **WHEN** a baseline that states no load is read
- **THEN** its load is treated as unknown rather than as zero
