## 1. Name why nothing came out

- [x] 1.1 Reproduce the audit's boolean: two field subtools added and dabbed with the default brush over a scaffold form; shown alone each evaluates far everywhere, and the run fails in `clay_item_volume_from_document` with "empty document".
- [x] 1.2 Add `BooleanRefusal::Formless` and refuse such an operand by name before sampling, with one evaluation at the middle of the region under the borrowed visibility.

## 2. Refuse at the choice

- [x] 2.1 Add `ObjectModel::admit_boolean_operand`, answered in the engine by `operand_kind` (gone, empty, hierarchy) and forwarded by `SharedDocument`.
- [x] 2.2 Replace the hierarchy's engine string with `BooleanRefusal::Hierarchy`, and have the run ask `operand_kind` for both operands before either is baked.
- [x] 2.3 Validate `SetBoolean` in `BooleanViewModel`: the same subtool twice, or an operand the model will not admit, writes the notice and leaves the pair as it was; a choice that is taken clears the notice.

## 3. Hold it

- [x] 3.1 `crates/clayspace-engine/tests/booleans.rs`: `a_boolean_produces_a_layer`, `a_formless_operand_is_refused_by_name`, `a_hierarchy_is_refused_when_it_is_chosen`, `an_empty_operand_is_refused_when_it_is_chosen`.
- [x] 3.2 `crates/clayspace-vm/tests/booleans.rs`: `an_invalid_operand_is_refused_at_set`, `the_same_subtool_twice_is_refused_at_set`, `a_good_choice_clears_the_last_refusal`, `a_refused_boolean_reports`.
- [x] 3.3 `docs/features.md`: when operands are checked, and what a relief-only subtool is.
