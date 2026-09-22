## 1. The history names the next step

- [x] 1.1 The sculpting ViewModel banks a label beside each action's entry count, and the label moves with the count between the undo and redo stacks.
- [x] 1.2 `SculptViewModel::next_undo` and `next_redo`: what the next step in each direction would take, rather than what last happened.
- [x] 1.3 Every edit banked from another ViewModel carries its own name onto the shared history — the command's label at the one place that banks them.
- [x] 1.4 `history.redo_depth` joins `depth`, and `Applied.undoes` answers the same question the section does.

## 2. The sections that were missing

- [x] 2.1 `brush`: the flow, the shaping controls, the grain in degrees, the six dynamics and what a drag reads.
- [x] 2.2 `combine`: the stroke's and the placement's, because they are two settings.
- [x] 2.3 `cage`, `deform`, `presentation`, `references`, `exchange`.
- [x] 2.4 `objects`: the placed forms, by the node id the selection is compared against.
- [x] 2.5 `outcomes`: the last rebuild, retopology and crossing, each absent until one has run.
- [x] 2.6 Nested where they belong: the grid's cells and passes, the hierarchy's levels, write domain and passes, and the scene's solo flag.

## 3. The fields that were wrong

- [x] 3.1 `scene.layers[].objects` counts placed forms on every representation; the grid's passes are reported under `passes`.
- [x] 3.2 `selected_object` is cleared when the node is no longer in the document's list.
- [x] 3.3 `tool.rig_mirror`, beside the brush's symmetry and only while a rig is being edited.
- [x] 3.4 `memory.cache_bytes`, the status area's own figure, beside the ledger's.

## 4. Hold it

- [x] 4.1 `StateQuery::NAMES` drives the reader, the refusal and the tool schema: one list, not three.
- [x] 4.2 `every_section_has_a_name_and_every_name_a_section`, and `every_named_section_is_a_key_the_report_carries`.
- [x] 4.3 `history_labels_name_the_next_step`, at the ViewModel and at the report.
- [x] 4.4 `a_cancelled_stroke_does_not_name_the_next_undo`, which is the defect in its own words.
- [x] 4.5 One test per new section against a fixture, in the crate that builds with no window, no GPU and no engine.
