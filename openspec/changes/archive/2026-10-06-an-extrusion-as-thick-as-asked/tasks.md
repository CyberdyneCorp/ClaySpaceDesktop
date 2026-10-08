## 1. The region (on ClayCore v0.120.1)

- [x] 1.1 `claycore::Document::layer_eval_points` / `layer_eval_gradients`.
- [x] 1.2 `extrude_region`: the side's distance band, the search box and its budget, the foot of a cell, a one-cell fill box; unit tests.
- [x] 1.3 `ClayDocument::extrusion_region` builds the swept mask; the field path extrudes from it.

## 2. Tests

- [x] 2.1 `mask_extrude_thickness.rs`: 0.05, 0.1 and 0.6 within 10% on an outline mask and on a painted mask; an even 0.6 wall across the patch; Centrado and Para dentro; a 100-unit wall refused.

## 3. Documentation

- [x] 3.1 `docs/features.md`: the extrude section, with before and after measurements and the voxel limitation.

## 4. On ClayCore v0.126.0 (#667, #691)

- [x] 4.1 Measure the swept region against the painted mask on the field fixtures: wall at 0.05, 0.1 and 0.6, evenness along the boundary, cost; decide.
- [x] 4.2 Remove the sweep: `extrude_region` becomes `extrude_budget` (the measurement's cell count and its budget only); the field path hands the engine the layer's own mask.
- [x] 4.3 `mask_extrude_thickness.rs`: the wall is even along the mask boundary on an outline and on a dab, at 0.1 and 0.6, within 0.005.
- [x] 4.4 `mask_extrude_thickness.rs`: a grid's wall at 0.05, 0.1 and 0.6 within one cell at the crown and across the patch while thin; even around the dab at 0.1 and 0.2 within half a cell; the porous 0.6 grid wall pinned as a tripwire.
- [x] 4.5 `docs/features.md`: the passage rewritten with both tables; the grid is no longer "still capped".
- [x] 4.6 The spec delta states the painted mask, the boundary evenness and the grid's crown and limitation.
