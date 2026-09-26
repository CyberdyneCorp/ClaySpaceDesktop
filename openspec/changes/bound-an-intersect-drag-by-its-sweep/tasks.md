# Tasks

- [x] Bind `clay_layer_set_transform_bound` in `claycore`.
- [x] Route a uniform-scale object move through it and refill what it reports;
      keep the per-axis path for a stretched object.
- [x] Reproduce the stale intersect placement on main, file ClayCore#665, and
      build a placed shape at its position in one edit.
- [x] Regression tests: an intersect drag frame refills about what the
      subtracting control does; the cache agrees with the document after the
      placement, the drag, a stretched drag, and undo/redo of the drag.
- [x] Update the documentation and the specification.
- [ ] Pass focused tests, lint, format, layering and OpenSpec validation.
- [ ] Pass CI.
