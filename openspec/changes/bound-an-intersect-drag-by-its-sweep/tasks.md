# Tasks

- [x] Bind `clay_layer_set_transform_bound` in `claycore`.
- [x] Route a uniform-scale object move through it and refill the overlap with
      the influence bound; keep the per-axis path for a stretched object.
- [x] Reproduce the stale intersect placement on main, file ClayCore#665, and
      build a placed shape at its position in one edit.
- [x] Regression tests: the cache agrees with the document after the
      placement, the drag, a stretched drag, and undo/redo of the drag; an
      intersect frame refills what the subtracting control does on the
      starting form, less than the layer on `reference` and well under half
      of it on `reference-10x`.
- [x] File the remaining engine-side cost, the chain pad (ClayCore#666).
- [x] Update the documentation and the specification.
- [x] Tighten the scaling test to the v0.126.0 answer (an intersect frame
      refills no more than its subtracting control) and measure both
      acceptance criteria on the new pin.
- [x] Pass focused tests, lint, format, layering and OpenSpec validation.
- [ ] Pass CI.
