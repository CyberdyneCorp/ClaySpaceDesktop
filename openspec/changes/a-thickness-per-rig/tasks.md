# Tasks

## 1. Thickness belongs to the rig

- [x] 1.1 Store the thickness on the layer, read each rig back through its
      own; verify `thickness_is_per_rig`
- [x] 1.2 Refuse a thickness without a rig and record nothing for an unchanged
      one; verify `a_thickness_is_refused_without_a_rig_and_unchanged_is_not_a_step`
- [x] 1.3 A thickness step names its layer; verify
      `undoing_across_a_thickness_does_not_compound_it` and
      `thickening_undoing_and_resizing_does_not_compound`

## 2. Saved with the document

- [x] 2.1 Write and read the `.rigs` side-car; verify the `rigs` unit tests and
      `a_rigs_thickness_is_saved_with_it`

## 3. Authoring edges

- [x] 3.1 `add` refuses a radius that is not positive; verify
      `add_refuses_a_negative_radius`
- [x] 3.2 `insert` is mirrored; verify `insert_is_mirrored`
