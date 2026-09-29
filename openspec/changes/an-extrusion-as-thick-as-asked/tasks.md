## 1. The region

- [x] 1.1 `claycore::Document::layer_eval_points` / `layer_eval_gradients`.
- [x] 1.2 `extrude_region`: the side's distance band, the search box and its budget, the foot of a cell, a one-cell fill box; unit tests.
- [x] 1.3 `ClayDocument::extrusion_region` builds the swept mask; the field path extrudes from it.

## 2. Tests

- [x] 2.1 `mask_extrude_thickness.rs`: 0.05, 0.1 and 0.6 within 10% on an outline mask and on a painted mask; an even 0.6 wall across the patch; Centrado and Para dentro; a 100-unit wall refused.

## 3. Documentation

- [x] 3.1 `docs/features.md`: the extrude section, with before and after measurements and the voxel limitation.
