## ADDED Requirements

### Requirement: A pull's taper is fixed when a point is placed
The Snake Hook authors a curve whose control points carry a radius each, and
the radius SHALL be a function of the distance travelled from the anchor.

It SHALL NOT be a function of the stroke's length. A taper measured across the
point index is renormalised by every additional sample, which changes the
radius of points the sculptor has already placed and is no longer touching —
measured, a point at index 5 thickened by 82% over one forty-sample pull.

This is a requirement about the *field*, not only about appearance. A radius
that can change behind the cursor makes the whole curve's field change on every
segment, which makes the whole curve's bricks correctly dirty and the pull
quadratic in its own length.

A pull longer than the taper's span SHALL hold its tip thickness rather than
re-thinning what is behind it.

#### Scenario: A pull is extended past a point already placed
- **WHEN** a Snake Hook pull is continued beyond a control point already sent
  to the engine
- **THEN** the field around that earlier point is unchanged, within the curve
  fitting tolerance

#### Scenario: A pull still reads as a tendril
- **WHEN** a pull is drawn out to full length
- **THEN** it is thicker at its root than near its tip

### Requirement: A live segment re-evaluates only what it changed
A segment of a gesture that grows an existing item SHALL dirty the region its
new samples changed, together with every image the layer mirror places that
region at, rather than the whole item's bound.

Each image SHALL be marked as its own region. A single box containing an image
and its reflection spans the untouched form between them.

Where the changed region cannot be named — the item did not grow, or this is
its first delivery — the item's own bound SHALL be used instead. Correct is the
direction to fail in, because the engine computes that bound in world space
from the document and it is right under every transform.

#### Scenario: A mirrored pull is drawn
- **WHEN** a segment of a pull is applied with a symmetry axis enabled
- **THEN** the reflection is dirtied in the same segment, and appears while the
  stroke is being drawn rather than when it ends

#### Scenario: A segment reports its own cost
- **WHEN** a segment grows a pull
- **THEN** the edit's dirty-brick count is what that segment dirtied, so that a
  profile of the stroke measures the segment rather than a constant
