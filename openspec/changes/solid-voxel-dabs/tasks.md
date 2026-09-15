## 1. Find what every brush shared

- [x] 1.1 Drive every voxel brush through the agent door on a converted sphere,
      each stroke undone and history checked back at 0 before the next
- [x] 1.2 Name the mechanism: `falloff x strength` dithered against a per-cell
      hash, with `seed: 0` on every dab, so overlapping dabs skip the same cells
- [x] 1.3 Rule out the half-size footprint theory — the field brush leaves a
      band of the same height (47 px) at the same radius

## 2. Choose the dab model on measurement

- [x] 2.1 A regression test that counts empty cells in the core of a default
      stroke, with a control that fails if the fixture cannot see holes
- [x] 2.2 Per-dab seed alone: 21.0% of the core still empty — rejected
- [x] 2.3 Threshold at half weight: shrinks a 9-cell dab to 3 at the default
      intensity and to 1 cell at 0.5 — rejected, it collapses a usable control
- [x] 2.4 Solid footprint with intensity as the bite: 0 of 377 core cells empty,
      against 237 of 377 on main — adopted

## 3. Land it

- [x] 3.1 `solid_footprint` for the stamping verbs and for the drag
- [x] 3.2 Keep the dither for an alpha carve, with a per-dab seed
- [x] 3.3 Guard that intensity still reaches the footprint, so a later change
      cannot make it inert unnoticed
- [x] 3.4 Correct `voxel_tools` and `voxel_display`, whose fixtures depended on
      the dither's pepper, and say so beside the correction
- [x] 3.5 `docs/features.md`: what Intensidade means on a grid
