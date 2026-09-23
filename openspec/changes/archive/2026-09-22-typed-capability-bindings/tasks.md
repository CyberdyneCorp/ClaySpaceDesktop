# Tasks

## 1. The typed row

- [x] 1.1 `SemanticIntent`, `ExecutionFamily`, `Fidelity` and `Binding` in
      `clayspace-model/src/tools.rs`, with the row shorthands that name the
      family at the call site (#205)
- [x] 1.2 `Verbs` carries `Option<Binding>` per representation; `on` answers
      with the whole binding and `entry_point_on` with the name alone (#205)
- [x] 1.3 Every row of both tables rewritten, plus the object row in `shape.rs`
      and the structural rows in the engine adapter (#205)
- [x] 1.4 `binding_on` beside `verb_on`; `exists_on`, `for_representation` and
      `availability` unchanged in meaning and now reading the binding (#205)

## 2. What the typed row makes checkable

- [x] 2.1 `every_binding_has_metadata` — the name parses, and the two ways a row
      can say "composed" agree (#205)
- [x] 2.2 `every_binding_is_filed_under_the_family_it_calls` (#205)
- [x] 2.3 `a_tool_means_one_thing_wherever_it_is_offered` (#205)
- [x] 2.4 `no_two_tools_on_one_representation_share_an_entry_point_without_differing_parameters`,
      with the two pairs that really are one binding named and their reasons
      given, and a failure when a recorded pair comes apart (#205)
- [x] 2.5 `a_note_never_hangs_off_a_row_that_does_what_its_label_says` (#205)
- [x] 2.6 `the_fields_standard_is_the_approximation_and_its_relief_is_the_inflate`,
      which ties the two rows, the fidelities and the caveat together as
      ClayCore v0.120.0 measured them (#205)
- [x] 2.7 `a_recipe_is_expressible_and_is_marked_as_one` (#205)

## 3. The report and the one-table rule

- [x] 3.1 `ToolDiagnostics` and the `tool:` line, read out of the capability
      table by the report itself (#205)
- [x] 3.2 The composition root fills it from the tool in hand and the active
      layer (#205)
- [x] 3.3 `the_tool_line_names_the_binding_its_intent_family_and_fidelity` and
      `a_tool_with_no_binding_on_the_active_layer_says_so` (#205)
- [x] 3.4 `tools/check_layering.py` fails a View, ViewModel, engine adapter or
      agent-facing crate that decides anything per (tool, representation) (#205)

## 4. What this change does not do

- It does not change which tools reach which representations, or which call any
  of them makes. The same shelf, the same refusals, the same engine calls; what
  moved is where the table says *what kind of call* each one is.
- It does not correct the two shelf entries that are one verb under two words —
  Padrão and Camada on a grid, Suavizar and Relaxar on a field. Correcting one
  is taking a tool off a shelf, which a sculptor feels and which a refactor of
  how the table is written is not the place for. They are recorded with their
  reasons, and the record fails when one of them comes apart.
- It does not bind a recipe. The family and the fidelity exist and are
  exercised by a test; the first composed tool — the engine documents
  DamStandard on a grid as one — arrives with the work that needs it.
- It does not build the advanced-help view. The table now answers the question
  that view would ask, per tool and per representation, which is what was
  missing.
