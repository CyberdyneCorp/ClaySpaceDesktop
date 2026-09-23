## 1. The order

- [x] 1.1 `App::dispatch_to_models` hands the command to the scene ViewModel before the sculpting, mask, cage and manipulator ones, and says why: the scene is the only one that moves the active layer and every one of them reads it.

## 2. The followers

- [x] 2.1 `SculptViewModel::dispatch` follows the active layer on `AddLayer` and `RemoveLayer` as well as on `SelectLayer`. A new layer arrives active and a removal hands the sculpt target to whatever is left.
- [x] 2.2 `MaskViewModel::dispatch` refreshes on the same three. None of them touches the document, so the composition root's post-edit refresh never ran for one.
- [x] 2.3 `SculptViewModel::refresh_after_conversion` and `refresh_after_open` become one `refresh_for_active_layer`, which is what both were.
- [x] 2.4 `Command::NewArmature` calls it, so the rig's own layer — created with its mirror off — is what the options bar shows and what the first ZSphere is placed with.

## 3. Hold it

- [x] 3.1 `crates/clayspace-vm/tests/viewmodel.rs`: `a_new_layer_arrives_with_its_own_brush_rather_than_the_previous_ones`, `the_first_stroke_after_a_switch_uses_the_new_layers_brush` — measured on the stroke the model was handed rather than on what the bar shows — and `a_new_rig_layer_uses_its_own_symmetry`. The double grows a per-layer mirror a test can move, as the document has.
- [x] 3.2 `crates/clayspace-app/tests/layer_switch.rs`: the followers against a real document — the brush comes back per representation, and a subtool with no mask reports none.
- [x] 3.3 The same file reads the composition root for the order and for the rig's refresh, the instrument `document_replaced_refreshes` uses and for the same reason: `App` lives in the binary and cannot be constructed without a window and a device.

## 4. Say so

- [x] 4.1 `docs/features.md`: when a switch takes effect, and that a new layer counts as one.
