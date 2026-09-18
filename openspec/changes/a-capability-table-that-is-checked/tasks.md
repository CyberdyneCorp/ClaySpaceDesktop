# Tasks

## 1. The names

- [x] 1.1 `claycore-sys/build.rs` writes down every function the generated
      bindings declare, read back out of `bindings.rs` rather than out of the
      header (#202)
- [x] 1.2 `claycore::ENTRY_POINTS` and `has_entry_point`, so a crate that links
      no engine can still have its strings checked by one that does (#202)
- [x] 1.3 `clayspace_model::entry_points` reads the names back out of a row, and
      `every_entry_point` gathers both tables (#202)
- [x] 1.4 `every_verb_the_table_names_is_a_symbol_the_engine_has` (#202)

## 2. The calls

- [x] 2.1 `claycore::trace`, recorded in `error::check` — the one function every
      fallible call passes — and compiled only under `test-support` (#202)
- [x] 2.2 `every_pair_calls_an_entry_point_its_row_names`, walking all four
      representations including the hierarchy (#202)
- [x] 2.3 Correct the rows it found wrong: the field's `_from` variants, the
      field drag's regions resolver, the grid's deposit, and the mesh and
      hierarchy rows that named the stamp where a resolved stroke runs (#202)
- [x] 2.4 Spell `clay_sdf_move_*` out in full, since an abbreviated name cannot
      be looked up (#202)

## 3. The notes

- [x] 3.1 `a_grid_flatten_fills_as_well_as_cuts`, against the scrape given the
      same plane (#202)
- [x] 3.2 `a_colour_brush_on_a_hierarchy_is_refused_for_real`, against the mesh
      route the note sends an artist to (#202)
- [x] 3.3 `a_hierarchy_smooth_does_not_pick_a_frequency_yet` — a tripwire, not a
      measurement, because the note is not true yet (#199)
- [x] 3.4 `every_tool_note_is_proved_here`, whose `match` stops compiling when a
      note arrives without one (#202)

## 4. What this change does not do

- It does not route the hierarchy smooth through the call that takes a mode.
  That is #199, and the row is left naming the right call because the row is
  right and the code is wrong. The tripwire in 3.3 and `DRIFTED_UNTIL_199` in
  `table_truth.rs` both fail the day it lands, and both say what to delete.
- It does not check the *brush kind* a row names in brackets — `(DRAW)`,
  `(FLATTEN)` — only the entry point. Sixteen mesh brushes reach the engine
  through one call, and telling them apart needs the descriptor traced as well
  as the call. `tool_table.rs` covers the ground in between: it asserts each
  declared pair changes the surface.
- It adds no CI job. `cargo test --workspace` already runs these, so a pin move
  runs them.
