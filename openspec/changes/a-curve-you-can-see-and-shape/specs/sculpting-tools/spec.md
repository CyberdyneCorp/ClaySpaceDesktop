## ADDED Requirements

### Requirement: A curve's guide is the line its tube follows
The viewport SHALL draw a placed curve's guide as the tessellation of its join,
not as the chords between its control points.

The two are the same line only for `Corners`. For a join that curves, the
chords cut the corners the tube rounds, and the guide is the only line the
sculptor can see — the tube conceals its own centre.

The tessellation SHALL be verified against the swept field rather than assumed
to match it. No ABI call returns a swept guide's tessellation, so the interface
computes its own, and agreement is a measurement: a sample on the guide reads
about minus the tube's radius, which is what the centre of a tube that thick
reads.

Membership SHALL NOT be accepted as that measurement. "Every sample is inside
the tube" is true of the control polygon as well, on any bend gentle enough
relative to the radius, and so cannot distinguish the right line from the wrong
one.

#### Scenario: A curve is drawn with a join that bends
- **WHEN** a curve using `Through` or `Rounded` is placed
- **THEN** the drawn guide passes through the tube's centre along its length,
  and not along the straight chords between the control points

### Requirement: A curve's guide is legible against its own tube
While a curve is being placed or edited, the surface SHALL be drawn ghosted, as
it is while a deformation cage is up.

The scaffold pass fades whatever the sculpt stands in front of. A cage's
control points are half behind the form; a curve's guide is *entirely* inside
the tube it describes, so without this the whole line is drawn faded.

Legibility SHALL be measured as the contrast the guide's pixels carry against
the surface behind them, not as the count of pixels it changes. The pass fades
the line rather than hiding it, so a count is cleared whether or not the fix is
present.

#### Scenario: A tube is swept along a curve still being edited
- **WHEN** the guide runs inside the tube it has swept
- **THEN** it reads against the tube rather than being faded into it

### Requirement: A curve can be refined between its points
A double-click on a curve's guide SHALL insert a control point that splits the
span under the pointer, selecting the new point.

Appending is what a click on empty space does. A curve that can only grow at
its end cannot be refined in the middle, which is where a tube usually needs
another point.

The span SHALL be resolved from the guide rather than the control points: the
second press of a double lands within a handle's reach of the point the first
one selected, so testing the points first would turn every double-click into a
re-selection.

#### Scenario: A sculptor double-clicks a bend
- **WHEN** a double-click lands on the guide between two control points
- **THEN** a control point is inserted between exactly those two, the points
  before and after it keep their places, and the new one is the selection

### Requirement: A curve can be drawn as well as clicked
A drag beginning where there is no control point and no guide SHALL lay a chain
of control points along the pointer's path, and a click in the same place SHALL
lay exactly one.

The two SHALL be told apart by distance travelled rather than by a mode. A press
that never moves never reaches the spacing, so one path serves both.

Spacing SHALL be measured in tube-widths. A point per frame is a curve that
cannot be edited afterwards, and being able to go back to it is what separates
this tool from a brush.

A freehand stroke SHALL stay on the plane its first point chose, and SHALL NOT
re-pick the surface per point: by the second point the tube the stroke is
drawing is under the pointer, so a ray cast at the surface lands on the stroke's
own output.

#### Scenario: A sculptor drags to draw a tube
- **WHEN** the pointer is dragged from a place holding neither a control point
  nor the guide
- **THEN** control points are laid along the path at tube-width spacing, and
  the tube follows them

#### Scenario: A sculptor clicks without moving
- **WHEN** a press begins and ends without travelling
- **THEN** exactly one control point is placed

### Requirement: A press on the guide does not extend the curve
A press landing on the guide, where no control point is under the pointer,
SHALL be consumed without adding a control point.

Appending there adds a point at the *end* of the curve — nowhere near the
pointer — and selects it, so the line appears to jump to a place nobody
clicked. It also makes a double-click on the guide unreachable, because the
first press of the double has already appended before the second can be read.

The order in which a press is resolved SHALL be decidable without a viewport: a
control point first, then a double on the guide, then the guide alone, then
empty space.

#### Scenario: A sculptor clicks the line once
- **WHEN** a single press lands on the guide away from any control point
- **THEN** the curve is unchanged and no point is added

#### Scenario: A sculptor double-clicks a control point
- **WHEN** a double-click lands on an existing control point
- **THEN** the point is taken in hand rather than a second one being inserted
  coincident with it
