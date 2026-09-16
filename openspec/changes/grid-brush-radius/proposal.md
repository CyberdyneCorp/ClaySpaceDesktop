## Why

A grid brush reached half as far as the same brush on a field. `clay_brush_params.size` is the footprint's span ACROSS — "cells the footprint spans per axis", in clay.h — and this application passed it the brush RADIUS. So every grid dab was half the ring the sculptor saw. Measured on a drag at brush size 0.4: a grid saturated at 0.20 where a field converged on 0.40, and the grid's ceiling was exactly half the brush size at two sizes (4 cells at 0.4, 8 cells at 0.8).

It was not found by reading either header — both say what they mean. It was found by driving the same nominal drag through both representations and comparing where the surface ended up.

## What Changes

- Both grid paths — stamping and the drag — pass `round(2 × size / cell)` as the footprint. The same brush now reaches the same distance on a grid and on a field.
- **Cost, accepted deliberately:** a dab's cells grow with the cube of its span, so every grid dab decides and writes roughly eight times the cells it did.
- **A ceiling that now binds sooner:** the footprint stays clamped at 64 cells across, so a grid brush stops growing past 32 cells of radius — 0.64 on a 0.02 grid — and the ring keeps moving past the dab. Documented rather than raised, since raising it multiplies the cost again.
- Fixtures corrected rather than retuned, each saying why beside the change:
  - The sign tests in `voxel_brushes` and the frozen-region test in `tool_table` counted mesh surface — indices, or fresh vertex positions — as a proxy for material. A dab at the true size carves through a thin slab and opens more surface than it removes, and a dithered nibble under a mask opens more surface than a clean scrape. Both now count **cells**. Measured, Padrão held inverted removed 1094 cells while its index count rose by 624; under a freeze Raspar removed 40 cells against 1060 unmasked (3.8%) while counting 483 fresh vertices against 972.
  - `voxel_dab_coverage` derived its core from the old rule, so the dab doubled around a core that stayed put. It now derives it from the new rule: 1469 core cells, 0 empty, against 377 before.
  - `brush_colour` strokes along the rod's surface rather than its axis, and saturates its freeze with repeated passes, since one Máscara pass tops out near 0.925.
  - `voxel_tools` punches single-cell cavities at size 0.025, which is one cell across under the new rule.
  - `voxel_grab_taper` probes its rim at 0.9 of the radius, because at 0.8 it passed exactly at its bound.
- The `voxel-reference` scene moves to revision `r3`, 12005 cells to 48805.

## Capabilities

### New Capabilities
- `sculpting-tools`: that a brush size means the same reach on every representation.

### Modified Capabilities

## Impact

- `crates/clayspace-engine/src/document.rs`: `stroke_voxel`, `voxel_grab_stroke`.
- Tests: `voxel_brushes`, `tool_table`, `voxel_dab_coverage`, `brush_colour`, `voxel_tools`, `voxel_grab_taper`; `crates/clayspace-app/src/reference.rs`.
- `docs/features.md`: the drag-reach table, the correction to the claim that the two representations already agreed, and the size ceiling.
- `benchmarks/baseline-linux-x86_64.json` records `voxel-reference-r1` and stops comparing until re-recorded on a quiet machine; the `brush.voxel.*` figures will move substantially.
