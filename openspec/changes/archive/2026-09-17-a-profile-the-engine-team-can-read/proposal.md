# A profile the engine team can read

## Why

This project measures a stroke carefully and then throws most of the
measurement away.

`SyncCost` already splits every re-mesh into five terms — the engine's
`clay_brick_cache_mesh`, our copy into the vertex layout, our per-key split,
our upload, and the shape of the work in keys and triangles — and
`SurfaceGeometry::sync` computes it on **every dab of every real stroke** in the
running application. `sync_geometry_now` then matches `Ok(_)` and drops it.
Only tests ever read `last_cost()`. What reaches the diagnostics report is a
single line, `re-malha 42 ms`, which is the one thing the ClayCore team cannot
act on: it does not say whether the 42 ms was theirs or ours.

The other half of the stroke is not measured live at all.
`SculptVm::apply_segment` calls `model.apply_stroke` untimed, and that call is
the purest ClayCore number this application could hand over — `dab_profile.rs`
says so in as many words: *"the engine applies the stroke and refills the bricks
it dirtied. Nothing of ours runs inside this."*

So today the answer to "is the dab slow, or the bake, or the meshing?" exists
only in `just budget`, `just segments` and the benchmark harness — on the
reference scenes, on our machine, on a build we made. It does not exist for the
document a sculptor is actually holding when it goes slow, and that is the case
worth reporting upstream. Every performance issue this project has filed was
reconstructed from a conversation, which is exactly the problem
`diagnostics.rs` was written to solve for versions and backends, and did.

There is one hazard to design against rather than to document away. An
unoptimised build runs this work about two and a half times slower —
`sculpt_latency.rs` refuses to assert a budget against one for that reason — so
a profile exported from a debug build and read as a ClayCore figure is worse
than no profile at all. The export must say what it is, loudly, in the file.

## What Changes

- **The engine's edit is timed.** `apply_stroke` gets a duration on the live
  path, so the two engine terms of a stroke — the edit and the refill, then the
  brick-cache mesh — are separated from each other and from ours.
- **`SyncCost` stops being discarded.** A `StrokeProfile` in `clayspace-model`
  accumulates every phase across the session: count, median, p95 and worst per
  phase, per tool, alongside the keys and triangles each sample covered. Held
  the way `FrameLog` is held — plain numbers, no clock, no engine types — so it
  is testable without a GPU.
- **The diagnostics report grows a stroke section.** Instead of `re-malha
  42 ms`, the pasted text says how those milliseconds divided between the
  engine and this application. The window shows the same table.
- **A JSON profile export.** **Ajuda → Exportar perfil…** (Help → Export
  profile…) writes one self-contained `.json` carrying everything the ClayCore
  team would otherwise ask for in a follow-up: the conditions (platform,
  architecture, engine version *and revision*, registered and active backends,
  selection reason, adapter, viewport), the document's shape (subtools,
  representations, layers, brick counts, triangles, memory by category), every
  per-phase distribution, every stall, every fallback, the per-pass GPU
  milliseconds, the measured refill cost per brick on each backend, and the
  build profile.
- **`refill_cost_per_brick` is finally plumbed.** It is documented "For
  diagnostics" and read by nothing but a test. It is the number behind the
  finding that CUDA is 3.5x slower than the CPU on the Linux reference machine,
  which is the single most ClayCore-actionable figure this project holds.
- **The file states its own trustworthiness.** A debug build stamps
  `"build": "debug"` and `"timings_comparable": false` at the top level, and
  the export dialog says so before writing. A release build stamps
  `"timings_comparable": true`.

## Capabilities

### New Capabilities
- `profile-export`: A single machine-readable file describing what a session
  cost and the conditions it cost it under, written for a reader who does not
  have the machine, the document, or the conversation.

### Modified Capabilities
- `diagnostics`: The report gains per-phase stroke attribution — which of a
  re-mesh's milliseconds were the engine's and which were ours — and the
  measured refill routing costs. Today it carries one total per command.

## Impact

- `clayspace-model`: new `profile` module beside `instrument`; `Diagnostics`
  gains a `stroke` section and a `refill` line. No new dependencies — this
  layer has none and keeps none.
- `clayspace-engine`: `BackendPolicy::refill_cost_per_brick` reaches a
  diagnostics accessor on `ClayDocument`; `apply_stroke` on `SharedDocument`
  returns or records its own duration.
- `clayspace-app`: `sync_geometry_now` folds `SyncCost` into the profile
  instead of dropping it; a JSON writer, hand-rendered rather than serialised,
  for the reason `bench/json.rs` gives — a serialiser in the dependency graph
  is a thing the licence audit carries forever for one file.
- `clayspace-vm`: a `ExportProfile` command and the timing around
  `apply_segment`.
- `clayspace-view`: a menu entry and a stroke table in the diagnostics window,
  in the three locales.
- Documentation: `README.md`'s *Acceleration and diagnostics*, and
  `docs/features.md`'s *Diagnostics*.
- **No measurable cost added to a stroke.** Every timer this change reads is
  either already running or is one `Instant::now` pair around a call that costs
  milliseconds.
