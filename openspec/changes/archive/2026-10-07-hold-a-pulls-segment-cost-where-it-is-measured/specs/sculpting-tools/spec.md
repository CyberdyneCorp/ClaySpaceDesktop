## MODIFIED Requirements

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

Over a long pull, the bricks a late segment dirties SHALL be held against the
bricks an early one dirties as an exact count, because the count is the same
on every machine and a segment that re-evaluates the whole tendril cannot
pass it. A segment's wall time SHALL be held beside the count as a class
bound and SHALL NOT be refereed as a late-over-early quotient while the late
figure is under the audit's own cheapest segment, 24 ms: a quotient of two
figures that small measures the host's scheduler rather than the code, and a
gate that fails on the runner's noise is one people learn to ignore.

#### Scenario: A mirrored pull is drawn
- **WHEN** a segment of a pull is applied with a symmetry axis enabled
- **THEN** the reflection is dirtied in the same segment, and appears while the
  stroke is being drawn rather than when it ends

#### Scenario: A segment reports its own cost
- **WHEN** a segment grows a pull
- **THEN** the edit's dirty-brick count is what that segment dirtied, so that a
  profile of the stroke measures the segment rather than a constant

#### Scenario: A late segment dirties the tip, not the tendril
- **WHEN** a 60-sample pull is delivered segment by segment with the dirty set
  drained before each one
- **THEN** the median of the last ten segments' dirty-brick counts is within
  1.5x the median of segments two to eleven

#### Scenario: A late segment's cost is refereed above the audit's regime
- **WHEN** the same pull's segments are timed, fastest of five takes
- **THEN** the late median is under six times the early one or under 24 ms,
  whichever is larger, and the figures are printed in either case
