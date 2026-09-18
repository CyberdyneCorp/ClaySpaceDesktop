## ADDED Requirements

### Requirement: A grid dab writes solid material
A dab on a voxel layer SHALL write its whole footprint, and intensity SHALL
scale the footprint's radius rather than the fraction of cells written.

Occupancy is binary, so a weight between 0 and 1 can only be spent as scattered
coverage. Spending it that way left the middle of a stroke porous, and with a
seed fixed for every dab the skipped cells were the same ones each time, so no
amount of overlap closed them.

An alpha carve is the exception: the stamp's own greys have nowhere but
coverage to live, so it still dithers, with a seed that differs per dab.

#### Scenario: A stroke at the shelf's defaults
- **WHEN** a Padrão stroke is made on a grid at the default intensity and
  falloff
- **THEN** no cell in the core of its footprint is left empty

#### Scenario: A lighter brush
- **WHEN** the same stroke is made at a low intensity and at full intensity
- **THEN** the lighter stroke writes materially fewer cells than the full one,
  and both write their core solid

#### Scenario: A drag on a grid
- **WHEN** Mover drags material on a voxel layer
- **THEN** the cells it carries are the whole footprint rather than a scattered
  subset of it
