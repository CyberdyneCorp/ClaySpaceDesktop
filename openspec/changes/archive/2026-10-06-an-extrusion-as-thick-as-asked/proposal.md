# An extrusion as thick as asked

## Why

The last two acceptance criteria of #178 (audit defect F5): Extrudar ignored
its thickness past how far the mask reached off the surface, and the wall's top
was lumpy, one lobe per dab. The engine of ClayCore v0.120.1 kept the part of
the wall's shell that lay inside the mask's own volume, read at the point itself
(`brush::mask_extrude`), and a mask painted on a surface is a thin volume around
it. Measured on the unit sphere with an outline mask, 0.6 gave a wall of 0.08 to
0.13; one Máscara dab of size 0.3 gave 0.13 to 0.16. The engine fix was filed as
CyberdyneCorp/ClayCore#660 and landed in ClayCore v0.126.0 (#667, with its cost
restored by #691).

## What changes

Two steps, one per engine pin.

On v0.120.1, the application worked around the engine on a field layer: instead
of the painted mask it handed the engine the region the fix would read — each
cell of the mask's bounds grown by the thickness, whose distance from the
layer's own surface lay in the band the side fills, took the painted mask's
value at its foot on that surface (`clay_layer_eval_points` and
`clay_layer_eval_gradients`, wrapped in `claycore`). The engine's intersection
then kept the whole shell and the top was the shell's offset surface.

On v0.126.0 the engine reads the mask at the source surface under each sample
itself, and the swept region buys nothing: measured on the unit sphere, the
painted mask gives the same 0.05, 0.1 and 0.6 walls to within 0.0002, as even
along the boundary (spread 0.0000 to 0.0009 on eight spots inside the outline's
square and eight around the dab's core), at 1.3 to 4.8 times less cost (outline
at 0.6: 2313 ms swept, 478 ms painted; dab at 0.6: 434 ms against 186 ms, Apple
M3 Pro). So the sweep and the code that built it are removed, and the field
path hands the engine the painted mask. The thickness tests stay as the
regression guard.

What stays from the first step is the budget: the engine measures the mask as a
dense array over its bounds grown by the thickness and the rim, and bounds
nothing, so the application counts those cells first and refuses past eight
million with a reason.

A grid extrudes through `clay_voxel_mask_extrude`, which v0.126.0 also
uncapped: the wall grows from each masked surface cell along an estimated
normal for the number of layers the thickness rounds to. Measured on the
starting form crossed to a 0.02 grid, the crown reads 0.060, 0.100 and 0.600
for 0.05, 0.1 and 0.6 (within one cell), and a wall up to ten layers is even
around the dab's core to within a quarter of a cell. Past that the one-cell
columns diverge and leave holes: at 0.6 the ring around the dab reads 0.35 to
0.41 against the crown's 0.600. That is an engine limitation of the voxel
extract, pinned by a tripwire test and documented, not worked around.

## Impact

`clayspace-engine`: `extrude_region` becomes `extrude_budget`;
`ClayDocument::extrusion_region` becomes `extrusion_within_budget`; the field
path extrudes from the layer's own mask. `claycore` keeps its two eval
wrappers. `docs/features.md` carries the measured tables. No file-format or
interface change.
