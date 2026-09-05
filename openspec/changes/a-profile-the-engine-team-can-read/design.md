## Context

See `proposal.md` — *Why*. Four facts about the current code shape the whole
approach:

1. `SyncCost` is **already computed on every dab of every live stroke**
   (`SurfaceGeometry::remesh`) and already carries the split this change wants
   to publish. `sync_geometry_now` matches `Ok(_)` and drops it. Nothing has to
   be instrumented to get four of the five phases — only kept.
2. The fifth phase, the engine's `apply_stroke` and brick refill, is called
   from `SculptVm::apply_segment` and is untimed. Every live path to it —
   the view model, the bench harness, the reference builder — goes through
   `SharedDocument::apply_stroke` in `crates/clayspace-app/src/shared.rs`,
   which is the composition root's own adapter and not a layer boundary.
3. `clayspace-model` has **no dependencies at all**, deliberately: that is what
   lets `diagnostics.rs` and `instrument.rs` be tested without a machine that
   happens to have the right hardware. Anything new that holds numbers belongs
   there under the same rule — durations passed in, no clock, no engine types.
4. There is no serialiser in the dependency graph, by decision.
   `bench/json.rs` states the reason: *"a serialiser in the dependency graph is
   a thing the audit has to consider forever for one file."* This change adds a
   second such file and does not get to reverse that.

## Goals / Non-Goals

**Goals:**

- One place that accumulates stroke phases, fed by both the engine edit and the
  re-mesh, readable by the diagnostics report and by the file writer.
- A file whose reader needs nothing else — no follow-up question about the
  build, the backend, the adapter, the document or the machine.
- Bounded memory over a session of any length, with the bound stated in the
  file rather than hidden.

**Non-Goals:**

- **A frame profiler.** The GPU per-pass timings already exist and are folded
  in as they are. Nothing here subdivides a render pass.
- **A flame graph, a trace format, or anything time-ordered.** This exports
  distributions, not a timeline. A timeline is a different file with a
  different reader and a much larger one.
- **Sending anything anywhere.** The file is written to a path the user chose.
  Nothing in this change opens a network connection.
- **Acting on the numbers.** Routing already acts on its own refill
  measurements. Nothing new here changes a decision the application makes.

## Decisions

### The profile lives in `clayspace-model`, beside `FrameLog`

`clayspace_model::profile::StrokeProfile`, holding plain numbers and taking
`Duration`s from its caller. Same rule as `instrument.rs`: the bookkeeping is
testable without sleeping, without a GPU and without an engine.

*Alternative rejected:* holding it in `clayspace-app` next to `SurfaceGeometry`,
which is where the numbers originate. That would put the arithmetic — merging,
quantiles, the retention rule — behind a GPU the test suite may not have, which
is precisely what the model layer's emptiness exists to avoid.

### One sample carries its phase, its tool and its workload

```
StrokeSample { tool, phase, took, work }
```

with `Phase` being `EngineEdit`, `EngineMesh`, `Read`, `Split`, `Upload`, and
`work` the size of what that sample covered — dirty bricks for the edit, keys
and triangles for the rest. Folded into `BTreeMap<tool, ToolProfile>`, one
`Samples` per phase inside.

Keyed by tool rather than aggregated flat because "the smooth brush is the slow
one" is the sentence an engine team can act on, and an aggregate over twenty-one
tools cannot produce it. The cross-tool aggregate is computed on read rather
than kept twice, so the two can never disagree.

*Alternative rejected:* a flat `[Samples; 5]` with no tool dimension. Cheaper,
and it throws away the only attribution that points at a specific engine kernel.

### Quantiles over a bounded retained window; the worst over the whole session

Each `Samples` keeps a ring of the most recent 4096 durations, plus an
unbounded `seen` count and an unbounded `worst`. Median and p95 are computed
exactly over what is retained; the file reports `seen` and `retained` as
separate numbers so a reader knows which population the quantiles describe.

An hour of sculpting is tens of thousands of dabs, and an unbounded `Vec` in
the interactive path is a leak with extra steps. A summary of count, sum and
max would be bounded but only yields a mean, which the spec forbids for the
reason it forbids it: a mean hides the tail a sculptor is complaining about.

*Alternative rejected:* a streaming quantile sketch. Approximate, and a
dependency, against a fixed 4096-sample window that is exact and is already
more strokes than a reportable session contains.

### Both feeds meet in `SharedDocument`

`SharedDocument` gains an `Rc<RefCell<StrokeProfile>>`:

- `SharedDocument::apply_stroke` times the engine call and records
  `EngineEdit` with `EditOutcome::dirty_bricks` as its workload. Every live
  stroke goes through here, so the view model needs no timing code and the
  layering is untouched.
- `sync_geometry_now` folds the `SyncCost` it already receives into the same
  profile — four records, one per phase.

*Alternative rejected:* timing inside `SculptVm::apply_segment`. It would put a
clock in the view-model layer and would still miss every path that reaches the
document another way.

### The file is rendered through a small writer, not a serialiser

A `Json` helper in `clayspace-app` that owns nesting, commas and string
escaping, so well-formedness is a property of the writer and not of forty call
sites. Point 4 of *Context* is the reason a serialiser is not used; the writer
is the smallest thing that makes hand-rendering safe rather than merely
customary.

Well-formedness is asserted structurally in tests — balanced containers, escaped
strings, every declared key present — rather than by parsing, since adding a
parser to assert the absence of a serialiser would be a joke at the audit's
expense.

*Alternative rejected:* `serde_json` behind a dev-dependency. Dev dependencies
are in the graph `deny.toml` walks, so this trades the stated reason for a
narrower version of the same cost.

### The file states its own trustworthiness rather than being withheld

`"build": "debug" | "release"` and `"timings_comparable": bool`, at the top
level where a reader hits them before any number. The export dialog says the
same thing before writing.

Refusing the export on a debug build was considered and rejected: the
identifying half of the profile — versions, revision, backends, fallbacks,
adapter, document shape, memory — is just as true in a debug build, and that is
often the build a contributor is running when they hit the thing worth
reporting. Withholding the whole file to protect five duration fields would
cost more than it saves.

### Redaction is a property of what is collected, not a pass over the output

No document path, no subtool name and no user content enters `StrokeProfile` or
the writer in the first place — a subtool is `{"representation": "sdf",
"index": 2}`. A redaction step applied on the way out is a step that can be
forgotten when a field is added; a collector that never holds the value cannot
leak it.

## Risks / Trade-offs

- **A debug profile is read as an engine figure anyway.** → Two independent
  markers in the file, a statement in the export dialog, and a note in the
  documentation. This is the failure mode the change exists to prevent and it
  is defended three times.
- **Timing overhead lands in the measured path.** → Two `Instant::now` pairs
  per dab, against phases measured in milliseconds. `sculpt_latency.rs` already
  gates median and p95 dab latency on a release build and is the guard against
  this regressing; no new gate is needed, but the existing one must stay green.
- **The 4096-sample window makes a long session's quantiles describe only its
  end.** → Accepted, and stated in the file: `seen` and `retained` are separate
  numbers precisely so this is visible rather than implied.
- **The file grows a field and the redaction guarantee quietly weakens.** → The
  test asserting that a document with named subtools exports none of the names
  is written against the whole rendered file, so a new field carrying a name
  fails it without anyone remembering to check.
- **Another window and another menu entry in an interface that already has
  many.** → It goes under Help beside Diagnostics, which is where a person
  looking for something to attach to a bug report already goes, and it is one
  entry rather than a panel.

## Migration Plan

Additive throughout. No stored file, no document format and no baseline changes
shape; `Diagnostics` gains fields and its `to_report` gains lines, both of which
existing readers tolerate. Rollback is removing the menu entry — the collection
is inert without a reader.
