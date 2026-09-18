## ADDED Requirements

### Requirement: A region can be frozen by drawing round it
The application SHALL offer two drawn gestures for the mask alongside the brush
that paints it: one that traces a shape freehand, and one that drags a
rectangle square to the screen from corner to corner. Choosing the gesture SHALL
NOT change which tool is in hand: it is a property of the mask brush, and all
three gestures write the same mask.

The two drawn gestures SHALL differ only in how the pointer builds the shape. A
traced outline SHALL follow the pointer's path; a rectangle SHALL be the box
between the point pressed and the point the pointer is at now, however far the
pointer wandered between them, and SHALL be the same box whichever corner it was
started from. What either produces SHALL freeze the same region.

An outline drawn over the viewport SHALL freeze everything it encloses on the
**active subtool**, and nothing outside it. The region SHALL be the outline
swept straight along the view direction, so the surface behind the outline is
frozen with the surface in front of it.

The region SHALL be bounded by the active subtool's own extent. Where the
subtool states no extent, the gesture SHALL be refused in words rather than
freezing nothing silently.

An outline that encloses no area — a click, or a drag that went out and came
back along its own line — SHALL do nothing, and SHALL NOT put a refusal on the
screen.

An outline drawn away from the form SHALL leave the mask as it was, and SHALL
NOT be reported as a failure.

#### Scenario: The enclosed side resists and the rest does not
- **WHEN** the user draws an outline around one side of the form and then
  applies a brush inside it and outside it
- **THEN** the enclosed side is unchanged and the side outside the outline moves

#### Scenario: The far surface freezes with the near one
- **WHEN** the user draws an outline over the form and then applies a brush to
  the surface behind it
- **THEN** that surface is unchanged

#### Scenario: A concave outline freezes what was drawn, not its bounding box
- **WHEN** the user draws an outline with a concave side, such as a C, and then
  applies a brush inside the opening
- **THEN** the surface in the opening moves, because it was never enclosed

#### Scenario: A click with a drawn gesture in hand does nothing
- **WHEN** the user presses and releases without drawing
- **THEN** the mask is unchanged and no refusal is shown

#### Scenario: A dragged box freezes what a traced outline round the same region does
- **WHEN** the user drags a rectangle over one side of the form, and separately
  traces an outline round the same side
- **THEN** the two freeze the same region

### Requirement: The same gesture releases what it encloses
Drawing an outline with the modifier that inverts a stroke held SHALL release
what the outline encloses rather than freezing it, leaving the rest of the mask
alone.

Which of the two it will do SHALL be decided when the gesture begins and held
for its whole length, as a stroke's modifiers are, so a key taken up part-way
round cannot change what the outline means.

Which gesture is drawing SHALL likewise be settled when it begins: changing the
gesture with the pointer down SHALL abandon what has been drawn rather than
reinterpret it.

#### Scenario: Changing gesture mid-drag abandons the outline
- **WHEN** the user begins an outline and then chooses another gesture without
  releasing
- **THEN** the outline is abandoned and the mask is unchanged

#### Scenario: Releasing part of a mask keeps the rest
- **WHEN** the user freezes a region and then draws an outline inside it with
  the invert modifier held
- **THEN** the enclosed part is released, the rest stays frozen, and a brush can
  reach the released part again

### Requirement: The outline is drawn while it is being made
The viewport SHALL trace the outline as the pointer draws it and SHALL
distinguish an outline that will freeze from one that will release.

Where the shape closes itself across a gap the sculptor can see — a traced
outline, whose ends need not meet — the viewport SHALL show the edge that will
close it, and SHALL show it as less certain than the edges actually drawn. A
rectangle has no such gap and SHALL be drawn as four equal edges.

The outline SHALL be taken down when the gesture ends, whether it was applied,
refused, or abandoned.

A gesture that begins off the form SHALL still be a gesture: pressing beside the
form with a drawn gesture in hand SHALL begin an outline rather than turning the
camera, since an outline is drawn *around* a region.

While a drawn gesture is in hand the brush ring SHALL NOT be drawn. A ring says
the next press leaves a stroke where it sits, and with a drawn gesture in hand
the next press draws a line on the screen instead — which the surface has no
footprint for.

#### Scenario: The line follows the pointer
- **WHEN** the user is part way through tracing an outline
- **THEN** the viewport shows the line drawn so far and where it will close

#### Scenario: A rectangle is drawn as a box
- **WHEN** the user is part way through dragging a rectangle
- **THEN** the viewport shows the box between the corner pressed and the pointer

#### Scenario: An abandoned outline leaves nothing behind
- **WHEN** the user abandons an outline in progress
- **THEN** no line is left on the viewport and the mask is unchanged

#### Scenario: The brush ring is off while a shape is being drawn
- **WHEN** the user chooses a drawn gesture and moves the pointer over the form
- **THEN** no brush ring is drawn, because no press there would leave a stroke

### Requirement: A whole lasso is one edit
A lasso SHALL reach the mask as a single recorded edit: one undo takes the whole
region back, and one redo puts it back, however many cells it covered.

The viewport SHALL re-sample the frozen region when a lasso lands, since a lasso
moves no clay and nothing else would prompt it.

The gesture SHALL be bounded in cost: a region too large to write in a bounded
time SHALL be refused in words that say what to do instead, rather than being
started and appearing to hang.

#### Scenario: One undo takes a lasso back
- **WHEN** the user freezes a region with a lasso and undoes once
- **THEN** nothing is frozen

#### Scenario: The frozen region is drawn straight away
- **WHEN** a lasso lands
- **THEN** the viewport draws the newly frozen region without any further edit

#### Scenario: A region too large to freeze at once says so
- **WHEN** the user draws an outline around the whole of a subtool large enough
  that freezing it would take an unbounded time
- **THEN** the gesture is refused with a reason, and the mask is unchanged
