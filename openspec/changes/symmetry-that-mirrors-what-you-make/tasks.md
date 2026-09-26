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

## 3. Remaining (#170)

- [ ] 3.1 Turning symmetry off or changing its axis leaves items made under the
      old mirror unchanged
- [ ] 3.2 A Move drag on the mirror plane moves as far as an unmirrored drag
- [ ] 3.3 Rig edits neither mirror one-sided spheres nor erase the rig layer's
      strokes
