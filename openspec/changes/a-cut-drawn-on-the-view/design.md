# Design

Four decisions, each with the alternative that was rejected and why. Three of
them are questions the engine deliberately leaves to the host, because it has
no viewport and does not want one.

## 1. The frame a shape is drawn on

`clay_cut_desc` takes an origin and three unit vectors — `right`, `up`,
`forward` — and the shape in **world units** on that frame. A non-orthonormal
basis is refused rather than squared up.

**Decision: the camera's own basis, with the origin on the near side of the
region being cut.** `right` and `up` are the camera's, so the shape a sculptor
drew on screen is the shape the cut has. `forward` is the view direction, which
is what makes the prism sweep away from the eye.

The origin is placed **outside** the region along `forward` rather than at the
form's centre, and `near_extent` / `far_extent` are left at zero so the engine
derives the sweep from `region_min`/`region_max`. The header says both zero is
"what a caller wants unless it is asking for a deliberate partial cut", and a
partial cut is not what this tool is.

**Rejected: a frame through the picked point.** It reads as more precise and is
worse — a cut whose depth depends on what happened to be under the pointer when
the gesture started is a cut that moves when a sculptor re-draws the same shape
from the same angle.

**The scale is the part that has to be got right.** Screen points must become
world units on that frame, which means the same projection the viewport used to
draw the overlay, at the depth the frame sits at. Under a perspective camera
that scale varies with depth — and the cut is a *prism*, so one scale has to be
chosen for the whole shape. It is taken at the region's centre depth, and the
consequence is written down rather than hidden: a shape drawn over a form that
is deep in the view cuts slightly wider at the front than the outline suggested.
An orthographic view has no such error, which is what the standard views are
for.

## 2. Which half survives

`clay_trim_side` says which half of the frame an **open** curve's outline
covers. The **op** decides that half's fate: `CLAY_OP_SUBTRACT` removes what
the shape covers, `CLAY_OP_INTERSECT` keeps only it.

**Decision: the side is inferred from the stroke's direction, and the op is the
sculptor's.** Walking the stroke left-to-right across the view takes what is
below it, as it does in ZBrush; the modifier that inverts every other brush
inverts this one, and inverting flips the **op**, not the side. Two controls
that both mean "the other half" is the second way to say one thing the engine's
own note warns about.

**Rejected: asking.** A dialog after every trim is not a sculpting tool.
**Rejected: always Subtract.** Intersect is how a form is cropped to a shape,
which is half of what a lasso is for.

A **closed** lasso has no side to infer — the outline is the shape — so
`clay_trim_side` does not enter, and only the op applies.

## 3. Two gestures, two calls

`clay_cut_polygon_from_open_curve` for a stroke drawn *across* the form,
`clay_cut_polygon_from_curve` for a closed lasso. The header warns that joining
a trim stroke's endpoints "cuts a sliver between them instead of dividing the
frame".

**Decision: the gesture chooses the call, and the sculptor chooses the
gesture.** `MaskGesture::Lasso` and `MaskGesture::Rectangle` already exist and
already mean "a shape traced over the form" and "a box square to the screen".
Trim gains the open-stroke gesture as a third, and each maps to exactly one
entry point.

**Rejected: one gesture with a "close it" toggle.** The two produce different
shapes from the same points, so a toggle would silently change what a drawn
line means.

`CLAY_CUT_RECT` exists and the rectangle gesture could use it directly rather
than a four-point polygon. It does — a rectangle is a rectangle, and going
through the polygon path would tessellate a shape the engine can express
exactly.

## 4. What a cut leaves behind

`clay_cut_create` returns a `clay_item*` the caller places like any other, and
the caller frees it with `clay_item_destroy`.

**Decision: the cut is an ordinary item in the active layer, and one undo
entry.** That makes a trim editable after the fact by the same route every other
placed item is, and it makes the tool's cost legible: a cut is an item, not a
bake.

**Rejected: resolving the cut into the layer immediately.** It would make a trim
cheaper to evaluate and impossible to adjust, and this application's whole
position on booleans is that a resolved operation is a choice a sculptor makes
rather than one a tool makes for them.

## What this change does not decide

**Whether a mesh subtool should be trimmable.** The verb table says SDF only,
because `clay_cut_create` resolves to a field item, so Trim greys out on a mesh
exactly as the table says it should. Trimming a mesh would mean crossing into a
field and back — a conversion with its own cost, its own undo entry and its own
loss — and folding that into a cut would hide a representation change inside a
tool that says it removes material. It is a separate proposal with a separate
argument, and this one states the limit rather than quietly widening it.
