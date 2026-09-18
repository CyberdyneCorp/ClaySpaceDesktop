# A curve you can see, and shape after you have drawn it

## Why

Tube along a curve places control points and sweeps a tube along them. Three
things about using it were wrong, and each hid the others.

**The line on screen was not the line the tube follows.** The overlay drew the
*control polygon* — straight chords between consecutive points — on the
reasoning that the sweep already shows the curve, so drawing it again would be
drawing the surface twice. That holds for a curve you can see. It fails for one
you cannot: the guide runs down the inside of its own tube, and under `Through`
(Catmull-Rom) or `Rounded` (B-spline) the chords cut the corners the tube
rounds. The one line a sculptor could see was the one the tube does not take.

**And it was dimmed.** The scaffold pass draws overlays wherever they are, but
fades whatever the sculpt stands in front of. A deformation cage switches the
surface to ghosted so its control points read through the form; a curve did
not, so a guide running down the middle of a tube was drawn at the faded alpha
along its entire length. Measured on one frame, the guide's pixels move by
**17.8** against an opaque surface and **55.0** against a ghosted one.

**And a curve could only grow at its end.** A click on empty space appends a
point. There was no way to put one *between* two others, which is where a tube
usually needs another — so refining a bend meant deleting back to it.

## What Changes

- `CurveState::path` tessellates the join, and the viewport draws that instead
  of the chords.
- The surface is ghosted while a curve is up, as it already is for a cage.
- A double-click on the guide splits the span under the pointer, through a new
  `insert_curve_point` and `Command::InsertCurvePoint`.

## Impact

**The guide is the tube's own centre line, and that is measured rather than
asserted.** There is no ABI call that returns a swept guide's tessellation, so
the interface computes it and `the_guide_lies_inside_the_tube_it_describes`
holds the agreement: every sample is evaluated against the swept field and has
to read about minus the tube's radius, which is what the centre of a tube that
thick reads.

That test found a real defect on its first strengthening. The first version
asserted only that samples were *inside* the tube, and it passed when fed the
control polygon — the very line the change exists to stop drawing — because a
chord across a gentle bend stays inside a tube of that radius. Measuring depth
rather than membership then failed on `Rounded` at exactly one sample of
thirty-seven: the last one, where the first draft pushed the final control
point on the reasoning that a curve ends on it. True of Catmull-Rom, which
interpolates; false of a B-spline, which does not reach its own endpoints.

The visual test went the same way twice. Counting changed pixels passed with
the guide dimmed *and* undimmed, because the scaffold shader fades the line
rather than hiding it. What separates them is how far those pixels move, so the
assertion is contrast rather than count.
