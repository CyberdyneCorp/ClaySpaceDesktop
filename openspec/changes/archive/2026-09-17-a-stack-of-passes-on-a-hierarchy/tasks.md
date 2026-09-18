## 1. The stack, read and written

- [x] 1.1 `Hierarchy::sculpt_layers` reads every row back from
      `clay_multires_sculpt_layer_*`, dropping a row that will not answer
      rather than refusing the read.
- [x] 1.2 `Hierarchy::active_pass` and `Hierarchy::stamps_into_a_pass`.
- [x] 1.3 `Hierarchy::apply_sculpt_layer_op` covering all eleven operations,
      each through the engine's own entry point.
- [x] 1.4 `ClayDocument::apply_multires_sculpt_layer_op`, refusing a layer that
      is not a hierarchy by name and a composition change while a gesture is
      open by what it is waiting for.
- [x] 1.5 Only the three operations `changes_the_surface` names re-derive the
      hierarchy's bounds.

## 2. The stroke

- [x] 2.1 `ClayDocument::stamp_into_a_pass`, through the layered transaction,
      committed explicitly rather than dropped.
- [x] 2.2 The path stamped sample by sample, with each sample's pressure
      reaching the stamp's strength, and the difference from a resolved stroke
      stated in the code and in `docs/features.md`.

## 3. The interface

- [x] 3.1 `Command::MultiresSculptLayer`, apart from `Command::SculptLayer`
      because the two stacks share no addressing.
- [x] 3.2 `SceneViewModel::apply_sculpt_layer_op`, through `finish`, so the
      refusal lands where every other scene refusal lands.
- [x] 3.3 The composition root runs it and redraws only where the surface moved.
- [x] 3.4 The stack drawn under the layer it stands on: a row per pass with an
      eye, a name, a strength and its marks, and the form's own row beneath
      them.
- [x] 3.5 Reordering by dragging a row's name onto another.
- [x] 3.6 The lock, the merge, the bake and the removal in the row's own menu,
      where a layer's protection already lives.
- [x] 3.7 A new pass, a compaction, what the stack costs, and why the
      composition is refusing.
- [x] 3.8 Nine strings in three locales.
- [x] 3.9 A failed save reaches the line beside the viewport.

## 4. Evidence

- [x] 4.1 `a_pass_is_still_dialable_long_after_the_stroke_that_filled_it`
      measures the surface at three strengths after the gesture closed.
- [x] 4.2 `sliding_a_pass_through_the_stack_moves_no_vertex` compares the drawn
      surface bit for bit.
- [x] 4.3 `selecting_the_form_leaves_the_passes_untouched`.
- [x] 4.4 Three refusals: a locked pass, a composition change mid-gesture, and
      a layer that is not a hierarchy.
- [x] 4.5 `the_hierarchys_passes_are_drawn_under_the_layer_they_stand_on` and
      `a_pass_row_asks_for_the_change_rather_than_making_it`.
- [x] 4.6 `dragging_a_pass_by_its_name_asks_for_a_reorder`.
- [x] 4.7 Three captures looked at rather than asserted about: the stack, the
      stack while a stroke holds it, and a save refused on the side-car.
