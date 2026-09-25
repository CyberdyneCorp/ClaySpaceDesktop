# Voxel Crease as an erode recipe

## Why

The grid offers Inflate but leaves Crease absent, although ClayCore can erode
with a negative Inflate amount. Sculptors need a narrow groove on voxel layers
without implying that the grid has a native crease or edge-sharpening verb.

## What changes

- Offer Crease on voxel layers as a pinned, narrow spherical erode recipe over
  `clay_voxel_sculpt_inflate`.
- Explain the recipe in the tool note and describe why voxel Smear and Clay
  remain unavailable.
- Pin the recipe parameters and groove result with regression tests.
