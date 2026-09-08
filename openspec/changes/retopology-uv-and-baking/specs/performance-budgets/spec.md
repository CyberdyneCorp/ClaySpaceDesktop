## ADDED Requirements

### Requirement: A second compute engine does not take the sculptor's cores
CyberRemesher's worker pool SHALL be bounded by `cyber_set_max_worker_threads`
before any of its work runs, and SHALL NOT be left at its default, which sizes
itself from hardware concurrency.

Their own note says an uncapped run "takes every core the host is trying to
share", and their cap is pinned by a test to produce byte-identical output — so
this costs a result nothing and costs a stroke everything.

The engine SHALL be built from the **`cpu-headless`** preset. ClayCore holds the
accelerated backend; two engines contending for one device mid-stroke is a
latency fault that cannot be read off a frame time.

#### Scenario: A retopology runs while the sculptor is working
- **WHEN** the operation is dispatched
- **THEN** it runs on a bounded pool and on the CPU, whatever the machine has

### Requirement: Retopology, UV and baking never run on the interface thread
Every CyberRemesher call SHALL be dispatched through `clayspace-vm::jobs`, which
already leaves the interface thread unblocked and **discards a result whose
document has moved on**.

Where the engine offers a cancellable entry point — `cyber_uv_atlas_cancellable`,
`cyber_uv_unwrap_seams_cancellable` — that one SHALL be used, and its progress
and cancel callbacks SHALL be wired to `jobs::Progress` and to the supersession
the module already implements.

An operation SHALL be refused while a gesture is open rather than queued behind
it, and the refusal SHALL say so.

#### Scenario: A document changes while a UV atlas is being computed
- **WHEN** the atlas returns
- **THEN** its result is discarded rather than written over the newer document

#### Scenario: A retopology is asked for mid-stroke
- **WHEN** a gesture is open
- **THEN** the operation is refused in terms a sculptor can act on

### Requirement: The sculpting figures are measured across the change, not asserted
The claim that sculpting does not slow down SHALL be evidenced by the figures
that already describe sculpting — `dab.*`, `brush.*`, `locality.*`, `tape.*` —
recorded **before the library is linked and again after, on the same machine in
one sitting**, and compared as a ratio with the machine's load recorded beside
it.

The comparison SHALL NOT be made against the committed baseline, which was
recorded at engine 0.52.2 and cannot separate this change from the engine
releases since.

A `retopo.*` group SHALL measure the new operations themselves, so they carry a
history from their first day.

`startup.*` and `memory.*` SHALL be read across the change as well: a second
shared object with its own OpenMP and TBB runtimes may cost something at load or
in resident memory that no per-operation figure reports.

#### Scenario: The before-and-after is taken
- **WHEN** the figures are recorded
- **THEN** both halves come from one machine in one sitting, with the load stated

### Requirement: The expensive quality mode is a knob, not a default
`--quality best` costs a **second full field solve**. It SHALL be offered as a
choice and SHALL NOT be the default, and the cost SHALL be stated where the
choice is made rather than discovered by a sculptor waiting.

**How much it costs here is unmeasured.** "Roughly double" is the engine
authors' estimate from what the code does, on their workloads, and they say so —
it is a knob to expose and then measure, not a figure to plan against. The
`retopo.*` group SHALL carry both settings so the number becomes ours rather
than inherited.

#### Scenario: The highest quality is chosen
- **WHEN** the option is presented
- **THEN** its cost is stated beside it
