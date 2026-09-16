## 1. Find it by driving both paths

- [x] 1.1 Sweep a drag's reach against the ask on a grid and on a field
- [x] 1.2 Pin the field's ceiling with asks well past its radius (0.3702 at 6.4)
- [x] 1.3 Confirm the grid's ceiling is half the brush size at two sizes

## 2. Correct the footprint

- [x] 2.1 Pass `round(2 × size / cell)` in `stroke_voxel` and `voxel_grab_stroke`

## 3. Correct what measured the old footprint

- [x] 3.1 Count cells, not indices, in `voxel_brushes`' sign tests — the brushes
      were right and the proxy read a carve as a deposit
- [x] 3.2 Count flipped cells, not fresh vertices, in `tool_table`'s frozen-region
      test — the mask held at about 4% and the proxy read it as a leak
- [x] 3.3 Re-derive `voxel_dab_coverage`'s core from the new rule
- [x] 3.4 Move `brush_colour`'s strokes onto the rod's surface and saturate its freeze
- [x] 3.5 Punch `voxel_tools`' cavities at one cell across
- [x] 3.6 Probe `voxel_grab_taper`'s rim at 0.9 of the radius for margin
- [x] 3.7 Bump `voxel-reference` to r3, 48805 cells

## 4. Say it

- [x] 4.1 `docs/features.md`: the reach table, the correction, the size ceiling
- [ ] 4.2 Re-record the benchmark baseline on a quiet machine
