# Hold the capability table to the engine that is linked

## Why

`ToolKind::verbs` is the authority for what each tool does on each
representation. The shelf reads it, the availability rule reads it, the
diagnostics report prints it and the tool notes hang off it — and until now
nothing could tell whether a row was true.

The only guard was `every_tool_names_an_engine_verb`, which asserted the string
began `clay_`. That passes for a renamed entry point, for a withdrawn one, and
for a verb the dispatch does not use. The other table tests check the table
against itself, which is worth having and is a different question.

Five kinds of row had gone quietly wrong under that guard:

- the field's smooth, its relax brush and its two planing tools named
  `clay_item_volume_relax` and `clay_item_volume_flatten`, the bake-then-act
  pair, where the code samples the document through the `_from` variants —
  which the engine says differ by "accuracy, and it is not small";
- the field's drag named `clay_layer_move_surface` where
  `clay_layer_move_surface_regions` runs;
- Padrão and Camada on a grid named `clay_voxel_sculpt_inflate` where the
  deposit runs, occupancy being binary and having nothing to inflate;
- fifteen mesh rows and twelve hierarchy rows named the stamp where a resolved
  stroke runs, which is a different entry point and not a flag.

A sixth is knowingly ahead of the code: the hierarchy's smooth names the call
that takes a frequency and no stroke opens it (#199).

This matters beyond tidiness. When the table drifts, the interface makes
promises the engine call does not keep — the same class of defect the audit
found on the agent-facing side, where the catalogue and the dispatch had come
apart.

## What changes

Two checks that the domain cannot make on its own, because `clayspace-model`
links no engine and holds its verbs as text:

- **The name is a symbol.** `claycore-sys` writes down every function its
  generated bindings declare, `claycore` publishes the list, and a test asks it
  for every name in the table. An engine rename is then a failing test on the
  pin move that does it, rather than drift.
- **The name is what runs.** Every fallible call in `claycore` passes through
  one function, so under `test-support` that function records the entry point.
  A test drives every offered (tool, representation) pair and fails a row whose
  stroke calls none of what it names.

The rows those checks found wrong are corrected. Each tool note gains a test
measuring the difference it describes, and the one note that is not yet true is
pinned so the day it becomes true is a failing test rather than a silence.

## Impact

- `sculpting-tools`: the verb requirement gains what "documented" now means,
  and a requirement is added for the two checks and for the notes.
- `crates/claycore-sys`, `crates/claycore`: the entry-point list and the trace.
  The trace is compiled only under `test-support`, so a sculpting session pays
  nothing for it.
- `crates/clayspace-model`: the corrected rows, and a reader that picks the
  names back out of one.
- `crates/clayspace-engine`: `tests/table_truth.rs`, which is where both halves
  are in scope at once.
- `docs/features.md`: the Engine verb column, and how it is now held true.
