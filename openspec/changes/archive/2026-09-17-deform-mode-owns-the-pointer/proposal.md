# A cage owns the pointer, and an arrow is grabbed by its shaft

## Why

Reported from using the deformation cage, with a screenshot: brush rings drawn
over the form while a cage was up, and *"the gizmo for movement only works if
we perfectly land the mouse on the axis arrow"*.

Both are the same kind of fault — what is drawn and what a press does had come
apart — and neither was visible to a test, because the picking lived in the
binary where nothing could reach it.

**The ring promised a stroke the routing refused.** `input::press_sculpts` has
refused to sculpt while a cage is up since the cage shipped, and there is a
test for it. The *cursor* was never told: `App::cursors` suppressed the brush
ring for the whole-subtool manipulator and not for a cage, so the sculptor saw
an orange brush over the very form the cage was there to bend and could not
tell whether a slip would leave a mark.

**Only the arrowhead could be grabbed.** The manipulator draws each arrow from
the pivot to its cone, and the hit test was a single sphere at the *tip* — a
target of `0.16` of the arm's length at the far end of an arm the sculptor can
plainly see. Worse than a miss: an axis ring encircles the pivot at `0.8` of
the reach, so a ray aimed down the inner shaft passes within a grab radius of
the ring's **far** side, and the press meant to slide the selection turned it
instead. Two of the three complaints about the manipulator — "nothing happens"
and "it rotates when I want to move" — are the same missing capsule.

**And a face could only be gathered a click at a time.** Turning or scaling a
cage needs two points or more, and a face of a `3x3x3` cage is nine of them —
nine chances to miss a `0.048`-wide target, with the brushes' own ring drawn
over half of them. A press that takes hold of nothing was spent on orbiting,
which the secondary button already does.

## What changes

- **The brush ring is drawn only where a press can leave a stroke.** One rule,
  `input::shows_the_brush_ring`, covering both modes that take the press away
  from the brush, instead of a check in the composition root covering one.
- **An arrow is hit-tested as a capsule from the pivot to its tip**, in the
  same nearest-along-the-ray competition as every other handle, and considered
  last so that a handle sitting *on* the shaft — the centre block, the scale
  box, the two rings that cross it — keeps its own press.
- **A press that takes hold of nothing while a cage is up draws a selection
  box**, and every control point inside it becomes the selection; Shift adds
  the catch instead of replacing. A press and release in one place is a click
  on nothing, which clears. The camera keeps the secondary button and the orbit
  modifier, which is what the old miss-orbits rule was for.
- The picking that decides all three moves out of `main.rs` into
  `clayspace-app`'s `input` module, where it can be — and now is — tested.

## What this does not change

The grab radius. `GIZMO_GRAB` is still `0.16` of the reach and `CAGE_GRAB`
still `2.2` handles: the arrows were not too thin, they were a sixth as long as
they looked. Widening every handle instead would have made the rings and boxes
steal from each other.

Picking stays in **world units** rather than screen space, for the reason the
cage picking already records: a screen-space radius makes a distant cage
unusable and a near one grab points it is nowhere near. The selection box is
the one part that is screen-space, because a box drawn on the screen is what it
is.

## The crossings, and who wins

A shaft is not alone: the centre block sits at its foot, the scale box at
`SCALE_BOX_REACH`, and two of the three rings cross every axis at `RING_REACH`
— each with the same grab radius. Testing the shaft last resolves this without
a special case, because the competition is already nearest-along-the-ray:

- a ring's **far** side is further along the ray than the shaft point in front
  of it, so the shaft takes a press aimed at the shaft — which is the bug;
- a handle **on** the shaft is the same distance from the eye, and a strict
  comparison leaves the press with whichever was considered first, which is the
  smaller and more particular target.

The same rule the outer ring has followed since it was added, and for the same
stated reason: the easy target must not steal the hard ones.
