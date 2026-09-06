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

### Requirement: Laying a control point costs the end it added
An appended control point SHALL dirty the region the new end changed, together
with every image the layer mirror places it at, rather than the swept node's
own bound.

A change that is not an append — a point moved or removed, a radius, join or
profile changed — SHALL use the node's own bound, since any of those can move
the whole tube.

The regions SHALL be marked separately rather than unioned into one box, which
would span the untouched form between an image and its reflection.

This SHALL be held by a test that detects **staleness**, not only cost: a brick
the append changed but the region did not name keeps its old value and nothing
reports it. Measuring the surface, refilling everything, and measuring again is
what distinguishes a region that named enough from one that merely named less.

#### Scenario: A curve is drawn freehand
- **WHEN** control points are appended one after another
- **THEN** each costs the end it added rather than the whole tube

#### Scenario: A curve laid point by point is compared with one refilled whole
- **WHEN** the layer is refilled from scratch after an incremental build
- **THEN** the surface is unchanged

### Requirement: The brush ring is not drawn while a curve is placed
The brush ring SHALL NOT be drawn while a curve is being placed or edited.

A ring under the pointer states that the next press leaves a stroke there. While
a curve is up a press puts a control point down, takes hold of one, draws a
chain of them, or is spent on the guide — none of which is a dab, so the ring
would be promising something no press there can deliver.

This SHALL be a clause of the rule that answers whether the ring is drawn,
alongside the whole-subtool manipulator, the deformation cage and the mask's
drawn gestures, rather than a condition at the call site: they are the same
question and a fourth answer kept somewhere else is how the third one came to
be missed.

#### Scenario: A curve is active
- **WHEN** the pointer is over the form with a curve being placed
- **THEN** no brush ring is drawn

### Requirement: Dragging a control point costs what the drag disturbed
A drag SHALL dirty the neighbourhood of the points that moved — both where they
were and where they now are — rather than the swept node's whole bound.

Both, because the field changed in both places: refilling only the destination
leaves the shape the point left standing on the surface with nothing to report
it.

The regions SHALL be one box per affected point rather than one box around the
range. A bent tube's enclosing box is mostly air.

The margin SHALL be measured against a **rendered** surface, not a pick. A pick
is answered from a path that stays correct whether or not the brick cache was
refilled, so it cannot detect a region that is too small — and the value that
merely passes a pick may sit below a real cliff.

Margins on different paths SHALL NOT be made to agree for tidiness. Each is
measured against its own cliff and its own cost, and the answers differ.

#### Scenario: A control point is dragged clear of where it started
- **WHEN** the surface is rendered after the drag, and again after every brick
  the tube reaches is refilled
- **THEN** the two pictures agree

#### Scenario: A margin is chosen
- **WHEN** a refill region's margin is set
- **THEN** the value sits above a cliff found by making the guard fail, and its
  cost on the path that runs most often is measured before it is widened
