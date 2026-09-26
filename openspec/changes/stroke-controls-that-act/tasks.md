## 1. Borda on a field

- [x] 1.1 `field_rim`: the stamp's rounding follows the falloff; Suave keeps the old rim.
- [x] 1.2 `falloff_changes_the_profile`: four falloffs, four profiles, in order of reach.

## 2. A segmented stroke is the gesture

- [x] 2.1 `SculptModel::stamp_gap`, forwarded by `SharedDocument`, overridden by `ClayDocument` for named field brushes.
- [x] 2.2 `ActiveStroke` moves to `stroke_path`: steadies the whole gesture, trims each stamping segment to where the next stamp is owed.
- [x] 2.3 The brush sent to the model carries no smoothing; drags are not steadied.
- [x] 2.4 Unit tests on the path; ViewModel tests for sparse strokes and smoothing; `stroke_continuity` against the real engine.

## 3. Documentation

- [x] 3.1 `docs/features.md` brush controls and mask extrude.
