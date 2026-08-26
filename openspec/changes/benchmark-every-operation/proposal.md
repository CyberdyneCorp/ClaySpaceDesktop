## Why

The performance gate measures one brush. `bench.rs` records twenty figures, and
every one of them that involves an edit is a `Padrao` dab on the reference SDF
scene. The application has twenty tools, six layer operations, deformers,
four conversions, voxel repair, masks, undo and export — and a ClayCore bump
can change any of them without moving a single recorded figure.

That is not a hypothetical. `stroke_budget.rs` carries a hand-maintained table
of gradient-normal cost across five engine versions because there was nowhere
else to keep it; `undo_cost.rs` exists because an undo cost 70x the dab it
reversed and nothing measured undo at all; `visual_brushes` prints a
per-segment cost for every tool and records none of it. Three separate places
have grown a private answer to the same question — *did this get slower?* — and
none of them survives being run on a different machine or compared across a
version pin.

The engine pin is about to keep moving. `just engine-pin <tag>` is a one-line
operation and the representations work is broadening what the application asks
of ClayCore. A gate that covers one twentieth of that vocabulary will report
green through a release that halves the speed of every region-based brush.

## What Changes

- **Every tool is measured.** One figure group per `ToolKind`, per
  representation the tool has a verb for, applied to that representation's
  reference scene. Stamping and region-based tools are separated, because they
  are different work and a single tolerance across both is meaningless.
- **Layer operations are measured**: taper, twist, lattice drag, close holes,
  fill voids, refine region — the `LayerOperation` set, each against the
  representation that carries it.
- **Deformers and rigging are measured**: the deform panel's operations applied
  to a layer, and authoring plus skinning a reference armature.
- **Conversions and bakes are measured**: the four directions across SDF, voxel
  and mesh, plus consolidation and export of the reference scene.
- **Masks are measured**: painting a mask, and the cost a gate adds to an
  otherwise identical stroke.
- **Undo and redo are measured**, as absolute cost and as a ratio against the
  edit they reverse — the ratio being the figure that survives a change of
  machine.
- **Reference scenes gain a voxel and a mesh form**, since a voxel verb has
  nowhere to land on an SDF document. `Scene` stops being one shape with two
  sizes and becomes a named suite, each scene revisioned as today.
- **The report and the baseline grow with them**, keeping one JSON file per
  platform. Comparison **reports a figure the baseline has and this run does
  not** rather than skipping it silently — a measurement that stopped running
  currently passes the gate.
- **The run is filterable and bounded**: `--only <prefix>` measures one group,
  and the full suite states its wall-clock cost so CI can budget for it.
- Figures arrive **without budgets**. The specification's budgets are stated
  for dab latency, frame rate, startup, memory and locality; a new figure is a
  tracked quantity, not a new promise. Budgets are added deliberately, later,
  where the application actually commits to one.
- Documentation: `stroke_budget.rs`'s cross-version table and `undo_cost.rs`'s
  narrative stay as explanations, but stop being the record — the record is the
  baseline file.

## Capabilities

### Modified Capabilities
- `performance-budgets`: **Performance is measured in CI, not asserted** grows
  from a named list of five measurements to a coverage rule — every operation a
  sculptor can invoke is measured, and adding an operation without a figure is
  a gap the gate can name. **A reference scene defines what the budgets are
  measured against** grows to a reference *suite*, one scene per
  representation, each naming its own revision. A new requirement covers what
  the comparison does about a figure that has gone missing.

## Impact

- `crates/clayspace-app/src/bin/bench.rs`: the measurement groups, the figure
  naming scheme, `--only`, and the missing-figure check in `compare`.
- `crates/clayspace-app/src/reference.rs`: `Scene` becomes a suite with voxel
  and mesh members; `Conditions` carries the suite revision rather than one
  scene name.
- `benchmarks/baseline-linux-x86_64.json`, `benchmarks/baseline-macos-aarch64.json`:
  re-recorded once on each platform, with the reason in the commit message.
- `justfile`: a recipe for the filtered run.
- Tests that duplicate what the gate will now record — `visual_brushes`'s
  per-segment print, `stroke_budget`, `undo_cost` — keep their assertions and
  stop being the only place a figure lives.
- CI: the benchmark job's runtime grows; the change states by how much.
- No engine-version floor change: everything measured is already reachable.
