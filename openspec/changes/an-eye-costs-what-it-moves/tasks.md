## 1. Ask once what the field holds

- [x] 1.1 `Layer::is_in_the_field` answers whether showing or hiding a layer can move a sample of the field, and says why two of the three representations cannot.
- [x] 1.2 `ClayDocument::write_layer_visible` marks nothing for a layer the field does not hold. The drain behind it is harmless where nothing was marked, so `SceneModel::set_layer_visible` keeps its shape.

## 2. A hop refills what the gesture moved

- [x] 2.1 `VisibilityGesture` carries `moved`: the layers the batch actually wrote a flag on, not the pattern it was asked for. `VisibilityWrites` keeps that list and the engine stamps together, so neither can grow without the other.
- [x] 2.2 `after_visibility_history` takes those layers, marks the ones the field holds and drains once — instead of refilling the active layer, which is not the layer whose eye moved. The drain is owed whether or not every mark landed, as it is in `write_visibility`.

## 3. A picture nobody is looking at is not built

- [x] 3.1 `resmooth_voxels` skips hidden grids, and says why it may while the chunk pass beside it may not: that one drains the engine's dirty set and skipping a layer would leave its keys queued.
- [x] 3.2 `ClayDocument::smoothed_grids` reports how many grids the last settle rebuilt, for the reason `meshed_chunks` exists.

## 4. Hold it

- [x] 4.1 `crates/clayspace-engine/tests/visibility_refill.rs`: `hiding_a_grid_does_not_dirty_the_field` and `a_grids_eye_still_reaches_what_is_drawn` — the second is what stops a cheaper hide that stops hiding anything from passing the first.
- [x] 4.2 The same file: `hiding_a_field_subtool_refills_it` and `showing_a_field_subtool_restores_the_same_surface`, asked of the **cache** and never of the document, since a refill that did not happen leaves the two disagreeing.
- [x] 4.3 The same file: `undoing_a_solo_refills_every_subtool_it_hid`, the regression for the active-layer refill. It fails on the previous body with the far subtools reading `None`.
- [x] 4.4 `crates/clayspace-engine/tests/voxel_display.rs`: `a_display_change_does_not_remesh_a_hidden_grid`, including that the deferred rebuild produces the same surface as a grid that was never hidden.

## 5. Say so

- [x] 5.1 `docs/features.md`: what an eye costs, and that a hidden grid's picture is built when it comes back.
