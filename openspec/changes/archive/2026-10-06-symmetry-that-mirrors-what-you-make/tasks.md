# Tasks

## 1. Items decide at creation

- [x] 1.1 Stroke stamps take part in the mirror only when the stroke's symmetry
      is on; verify `turning_symmetry_on_leaves_existing_items_alone` and
      `a_stroke_made_with_symmetry_on_is_mirrored`
- [x] 1.2 Snakehook tendrils carry the same decision
- [x] 1.3 Placed objects carry it, and point the mirror in the placement's undo
      group when symmetry is on; verify
      `a_placed_object_is_not_duplicated_by_a_later_symmetry_change` and
      `a_placed_object_is_mirrored_when_symmetry_is_on`
- [x] 1.4 Curves carry it and point the mirror of the layer they were begun on;
      verify `a_curve_begun_with_symmetry_on_is_mirrored` and
      `a_curve_made_one_sided_stays_one_sided`

## 2. No stale surface after a mirror change

- [x] 2.1 A real mirror change marks the layer under the old and the new
      mirror and drains once; verify `a_mirror_change_dirties_both_images`,
      `a_hidden_layer_draws_nothing_after_a_mirror_change` and
      `undoing_a_mirror_change_draws_the_old_images_again`

## 3. Rigs (#170, A5)

- [x] 3.1 The armature item stays out of the layer mirror; verify
      `a_stroke_under_symmetry_does_not_mirror_a_one_sided_zsphere`
- [x] 3.2 A rewritten armature goes back to where it stood in the layer's
      order, in the edit's undo group; verify
      `a_rig_edit_keeps_the_strokes_on_the_rig_layer`

## 4. Symmetry off keeps the mirror (#170)

- [x] 4.1 Item-adding and bake verbs made with symmetry off leave the layer's
      mirror as it stands; verify `turning_symmetry_off_leaves_mirrored_items_alone`,
      `a_pull_with_symmetry_off_leaves_mirrored_items_alone` and
      `a_bake_with_symmetry_off_is_not_copied_across_a_kept_mirror`
- [x] 4.2 Move still writes the mirror off, so a drag with symmetry off moves
      one side; verify `a_drag_with_symmetry_off_moves_one_side`
- [x] 4.3 The mirror-change and mirror-after-undo tests drive a change of axis,
      which still writes the mirror, instead of turning it off

## 5. Per-item axes (#170, ClayCore v0.126.0)

- [x] 5.1 Every item carries the axes it was made under, so changing the
      axis, or a Move or Pinch with symmetry off, leaves items made under
      the old mirror unchanged; verify
      `switching_the_axis_leaves_items_made_under_the_old_axis_unchanged`,
      `turning_symmetry_off_leaves_a_mirrored_item_mirrored`,
      `a_drag_with_symmetry_off_on_an_item_made_under_symmetry_moves_both_images`,
      `an_item_made_under_symmetry_is_mirrored_on_a_layer_whose_mirror_is_off`
      and `a_reopened_document_keeps_each_items_own_axes`; the `claycore`
      wrappers are verified by `item_mirror_axes.rs`
- [x] 5.2 A Move drag on the mirror plane moves as far as an unmirrored drag;
      `a_move_on_the_plane_is_applied_once` is no longer ignored
