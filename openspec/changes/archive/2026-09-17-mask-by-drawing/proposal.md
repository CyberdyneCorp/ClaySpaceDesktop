# Freeze a region by drawing round it

## Why

Masking has one gesture: drag the brush over the surface and it freezes what it
touches. That is the right gesture for a soft, painterly freeze and the wrong
one for the thing masking is most often wanted for — *this limb, not that one*,
*everything above this line*. ZBrush answers that with the lasso and the rect,
and they are not a convenience: a shape drawn on the screen freezes what it
encloses **through the form**, front and back, in one gesture, with an edge
exactly where the sculptor drew it. A brush cannot express that at all. It
cannot reach the far side without turning the model, it cannot give a straight
edge, and freezing "the top half" with it is a minute of scrubbing that ends in
a ragged boundary.

The rectangle is not a lesser lasso. A hand cannot draw a straight line, and
"everything above this line" is the most common thing a mask is wanted for —
which is why ZBrush ships both and why the two share everything below the
pointer.

The specification already names the shape of the gesture, in the entry beside
this one: *"a shape drawn on the view frame — rectangle, circle, polygon or
lasso"*, for the cut tool. The engine names it too, in `clay_cut_desc`: an
origin, an orthonormal basis, and an outline in world units on that frame,
swept straight. Nothing in this application has ever built one.

## What changes

- The mask brush gains two more **gestures**: `Pincel`, the drag it has always
  had, `Laço`, a shape traced freehand over the form, and `Retângulo`, a box
  dragged corner to corner and square to the screen. One setting on one brush
  rather than three tools, which is where ZBrush keeps them and what avoids a
  second answer to every availability question the first one already answers.
- Drawing a shape freezes everything it encloses on the **active subtool**,
  through the form: the far surface behind the outline freezes with the near
  one, because the outline was drawn on the screen.
- The two drawn gestures differ in exactly one place — how the pointer builds
  the shape. A lasso keeps every point it passed through; a rectangle keeps the
  corner it started at and replaces the other. Past that they are the same list
  of points, and neither the containment test, the traversal nor the engine can
  tell them apart.
- Either gesture with the invert modifier held **releases** what it encloses,
  which is how a mask is trimmed back without clearing it.
- The outline is drawn over the viewport while it is being made, in the accent
  when it will freeze and in a dim neutral when it will release. A lasso's
  closing edge is shown faint, because it closes across a gap the sculptor can
  see; a rectangle's four edges are all solid, because it has no such gap.
- A whole gesture is **one undo**, and the viewport re-samples the frozen region
  when it lands.

## How the region reaches the mask

Not as cells. A document-owned mask snapshots its whole chunk map for the undo
history on **every** call that writes to it, and diffs it again when the call
returns. Measured on a mask covering a million cells: about four milliseconds a
call, so a region delivered as five thousand one-cell fills takes twenty-one
seconds. Against seven microseconds for the same call on a standalone mask,
which records nothing — the cost is the history, not the write.

| delivery | calls | time |
|---|---|---|
| 5000 one-cell fills, document mask | 5000 | 21.2 s |
| 5000 one-cell fills, standalone mask | 5000 | 35 ms |
| the whole reference form as one stamp run | 1 | 659 ms |

So the region is delivered as **one stroke**: a path that visits every column of
it, walked by the engine's own arc-length stamper. One call, one snapshot, one
undo entry. The path is a depth-first walk of the lattice the outline covers,
and it never leaves the region — a connector that cut across a concave outline
would freeze a stripe nobody drew, which is exactly what a plain back-and-forth
over the rows does the first time a lasso is drawn round a C.

## What this costs, and what it does not fix

**A lasso costs the volume it sweeps.** The lattice the path is walked on is
aligned to the *camera*, because that is where the outline was drawn, and a
brush footprint is aligned to the *world*. A ball has to reach half the pitch's
diagonal to cover the lattice from any angle rather than half its side, so the
balls overlap and every cell of the region is written about 2.7 times — about
140 nanoseconds a write, measured. Sized to half a side instead, the two tile
only when the camera happens to face down an axis, and from anywhere else the
frozen patch comes out speckled with cells no stamp reached. A cube of the same
reach writes 5.8 cells per region cell rather than 2.7, all of the difference in
corners that overshoot the region; a ball is worth 40% of the gesture, measured
at 1191 ms against 800 on the reference form on one machine.

**The pitch is not a dial.** Opening it by two divides the stamps by eight and
multiplies the cells each writes by eight, so it changes nothing but how coarsely
the region's edge is quantised — two mask cells, 0.04 world units at the mask's
own 0.02 pitch. So it is fixed at the finest pitch worth walking, and a region
too large to write at all is **refused with a reason** rather than pretended
away by coarsening it: `mask.outline` on the reference form, a lasso thrown around
the whole of it, is 659 ms on a quiet machine, and the ceiling is a little under
two seconds' worth of writes.

**The engine has no bulk mask write.** `clay_mask_set`, `clay_mask_paint_cell`
and `clay_mask_fill` each bracket their own undo step, and there is no entry
point that takes a set of cells, a second mask to merge, or a mask as a brush
gate — `clay_mask_paint` documents that `brush->mask` is ignored, "a mask does
not gate itself". Everything above follows from that. Recorded in
`docs/roadmap.md` under the engine's known costs.

The lasso is a **prism**, not a cone, under a perspective camera — the engine's
own reasoning for the cut tool, *"a trim is a straight cut, as it is in ZBrush
and 3DCoat"*. A region defined by a cone would depend on where the camera was
standing.

Symmetry does not reach it, exactly as it does not reach the mask brush: a mask
is a world-addressed field and neither gesture is mirrored.

`mask.outline` is measured but **not recorded in the baseline**. That file was
taken against ClayCore 0.52.2 and the tree is pinned to 0.60.0, so
`bench-compare` is already red on `main` — verified by running it on a clean
checkout, where it reports more regressions than it does here. A figure spliced
into it now would be a number nobody could read a comparison against; it goes in
when the baseline is re-recorded for 0.60.0. A figure the baseline lacks is
reported as `new` rather than failing the gate.
