# An extrusion as thick as asked

## Why

The last two acceptance criteria of #178 (audit defect F5): Extrudar ignored
its thickness past how far the mask reached off the surface, and the wall's top
was lumpy, one lobe per dab. The engine keeps the part of the wall's shell that
lies inside the mask's own volume, read at the point itself
(`brush::mask_extrude`), and a mask painted on a surface is a thin volume around
it. Measured on the unit sphere with an outline mask, 0.6 gave a wall of 0.08 to
0.13; one Máscara dab of size 0.3 gave 0.13 to 0.16. The engine fix is filed as
CyberdyneCorp/ClayCore#660 and is still open.

## What changes

- On a field layer, Extrudar no longer hands the engine the painted mask. It
  builds the region the engine fix would read: each cell of the mask's bounds
  grown by the thickness, whose distance from the layer's own surface lies in
  the band the side fills, takes the painted mask's value at its foot on that
  surface. The engine's intersection keeps the whole shell and the top is the
  shell's offset surface.
- Distances and normals come from the layer alone (`clay_layer_eval_points`,
  `clay_layer_eval_gradients`, newly wrapped in `claycore`), so another subtool
  beside it — an earlier extrusion — cannot bend them.
- A thickness whose search box would exceed 8 million cells is refused with a
  reason, rather than handed to an engine whose own dense measurement covers the
  same box.

## Out of scope

A voxel layer's extrusion grows cell by cell through masked cells in the engine
and has no layer field for a normal, so it stays capped until ClayCore#660
lands. The app then needs a pin bump; the region built here stays correct under
the fixed engine (it is the region the fix reads).

## Impact

`claycore` (two safe wrappers), `clayspace-engine` (`extrude_region`,
`ClayDocument::extrusion_region`). No file-format or interface change.
