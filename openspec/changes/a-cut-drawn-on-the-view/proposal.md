# A cut drawn on the view

## Why

**Trim is on the shelf and does nothing.** `ToolKind::Trim` is in the tool
table, its verb row already names `clay_cut_create` as the SDF binding, and its
doc records the design — *"its gesture is a shape drawn on the view frame,
resolved into a prism that cuts through. Treating it as a surface stroke would
be a tool that looks available and does something else."* But `operation()`
returns `None` for it, `clay_cut_*` appears nowhere in the `claycore` wrapper,
and the benchmark baseline records `brush.sdf.trim` as skipped with *"no
gesture this harness can synthesise."*

So a sculptor can pick it. It is a shelf entry with an intent written down and
nothing behind it, which is the state the tool table exists to make visible and
this change exists to end.

**The engine has every piece, and its shape is not the obvious one.** The cut
is a **prism, not a frustum** — the header is explicit that a shape drawn under
a perspective camera sweeps a converging wedge, and cutting with one gives a
cut face that is not flat and a solid that depends on where the camera stood.
A trim is a straight cut, as it is in ZBrush and 3DCoat.

**And the engine has no viewport and does not want one.** It takes the *frame*
the shape was drawn on — an origin and an orthonormal basis — with the shape in
**world units** on that frame, not pixels and not normalised device
coordinates. A frame that is not orthonormal is refused rather than squared up,
"because the shape the user saw was drawn in the frame they think they have".
That puts the whole screen-to-world question on this side of the wire, which is
where it belongs and where the work in this change actually is.

**Two gestures, two entry points, deliberately not one call with a flag.**
`clay_cut_polygon_from_open_curve` is ZBrush's Trim Curve: an open stroke across
the form, flattened and closed against the frame's own bounds on the side it
covers. `clay_cut_polygon_from_curve` tessellates **closed** and is a spline
lasso. The header names the trap: joining a trim stroke's endpoints "cuts a
sliver between them instead of dividing the frame". Different shapes from the
same points.

**And the half that survives is the op, not a flag.** `CLAY_OP_SUBTRACT`
removes what the shape covers; `CLAY_OP_INTERSECT` keeps only that. The
`clay_trim_side` enum says which half of the frame an *open* curve's outline
covers — it does not decide that half's fate. Reading those as one thing is the
mistake this proposal exists partly to write down.

## What Changes

- `claycore` gains safe wrappers for `clay_cut_create`,
  `clay_cut_polygon_from_open_curve` and `clay_cut_polygon_from_curve` — the
  only crate that may hold the `unsafe`.
- `ToolKind::Trim` is routed through the **drawn-outline** gesture the mask
  brush already has, not the stroke path.
- The interface resolves the drawn shape into a world frame and a polygon in
  world units on it, and places the resolved item with Subtract or Intersect.

## Impact

**Most of this already exists at both ends, and the work is the middle.** The
gesture half is built and shipped: `MaskGesture::Lasso` and
`MaskGesture::Rectangle`, `BeginMaskOutline` / `ExtendMaskOutline` /
`EndMaskOutline`, and the brush-ring suppression that goes with a drawn
gesture. The op half is built: `Combine::Subtract` and `Combine::Intersect` are
what our boolean already places operands with.

What is missing is the binding and the crossing between them — screen points to
a world frame, and a decision about which half a sculptor meant.

**SDF only, and that is a refusal rather than a gap.** The verb table gives
Trim `sdf: Some("clay_cut_create")` and `None` for voxel, mesh and multires,
because `clay_cut_create` resolves to a field item. A mesh subtool offers no
Trim, and the tool greys out rather than doing something else — which is the
rule the table exists for. Whether a mesh should be trimmable by crossing into
a field and back is a question this change **states and does not answer**; it
is a conversion with its own cost and its own undo entry, and folding it in
here would hide a representation change inside a cut.
