## Why

Every voxel brush left the same crusty speckle, which is what "voxel sculpting
seems a bit useless, all the brushes give a similar strange result" meant
(#139). Occupancy is binary, so ClayCore spends a fractional weight by
dithering: it writes a scattered subset of the footprint chosen by a hash of
each cell's coordinate and the brush's seed. The application sent the shelf's
defaults — 0.65 intensity, a smooth falloff — and `seed: 0` on every dab, so
63% of the cells in the middle of a stroke were skipped, and the *same* cells
were skipped by every later dab. A stroke could be crossed any number of times
and never fill in.

## What Changes

- A grid dab is written **solid**: constant falloff, full strength, with
  intensity scaling the dab's radius between half and full instead of its
  porosity. A lighter brush takes a smaller bite, which is the only reading of
  "lighter" binary occupancy can honour.
- The voxel drag (`clay_voxel_sculpt_grab`) takes the same footprint, because a
  dithered drag carries only some of the cells it reaches and tears the lump.
- An alpha carve keeps its dither — a stamp's greys have nowhere else to go on
  binary cells — but gets a **seed that differs per dab**, so a dragged stamp
  fills in rather than repeating its own holes.
- Two fixtures that depended on the defect are corrected rather than retuned:
  `voxel_tools` punched no cavities of its own and relied on the dither's
  pepper for Preencher to close; `voxel_display` read dither specks as the
  detail its blur was meant to take down. Both now build their subject on
  purpose.
- **Not** changed: what intensity means on a field or a mesh, where partial
  density is representable and the existing reading is correct.

## Capabilities

### New Capabilities
- `sculpting-tools`: what a dab writes on a grid, and what intensity means when
  a cell can only be full or empty.

### Modified Capabilities

## Impact

- `crates/clayspace-engine/src/document.rs`: `stroke_voxel` and
  `voxel_grab_stroke`.
- Tests: `voxel_dab_coverage.rs` (new), `voxel_tools.rs`, `voxel_display.rs`.
- `docs/features.md`: Brush controls.
- Closes #139.
