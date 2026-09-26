## Why

Several agent commands report success when there is no target, and two view commands do not perform their advertised effect. A partial coarse brick set also leaves an edited model with missing, flat regions.

## What Changes

- Report `nothing_to_do` as a distinct successful outcome with a reason for harmless commands that have no target.
- Make the grid overlay follow its toggle, copy diagnostics through the system clipboard, and frame visible subtools only.
- Use reduced LOD only when its coarse bricks cover the whole visible field; otherwise render the complete full resolution surface.
- State and test the mask rule for mesh taper and twist deformers.

## Impact

Agent responses gain an `outcome` field. A field with incomplete mip coverage may remain at full resolution until complete mip coverage is available, trading rendering cost for a complete image.
