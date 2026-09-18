## ADDED Requirements

### Requirement: Every part of a handle a person can see can be grabbed
A press on any part of a manipulator arrow — anywhere along the shaft as drawn,
not only its arrowhead — SHALL begin a slide along that axis. A press past
either end of the arrow SHALL NOT.

Where another handle sits on the shaft — the centre, a scale box, or a ring
that crosses the axis — that handle SHALL keep the press. Where a ring passes
*behind* the point pressed, the shaft SHALL take it: the handle nearer the eye
is the one a person aiming at what they can see means.

What is drawn and what can be grabbed SHALL come from one definition, and the
rule SHALL live where a test can reach it rather than in the composition root.

#### Scenario: A press halfway along an arrow moves the selection
- **WHEN** the user presses the vertical arrow halfway between the pivot and
  its arrowhead, clear of the scale box and the rings
- **THEN** the selection slides vertically

#### Scenario: A ring behind the press does not take it
- **WHEN** the user presses a point on the inner shaft, with the far side of an
  axis ring behind it along the same ray
- **THEN** the selection slides rather than turning

#### Scenario: The handles on the shaft keep their presses
- **WHEN** the user presses the scale box, or the centre block at the arrow's
  foot
- **THEN** that handle's operation runs, not a slide along the axis

#### Scenario: A press past the arrowhead grabs nothing
- **WHEN** the user presses well beyond the tip of an arrow, along the same
  axis
- **THEN** no handle is grabbed

### Requirement: The handle a press would take is shown before the press
The manipulator SHALL light the handle under the pointer, answering the same
question a press answers and from the same rule, so that what is highlighted
and what a press grabs cannot describe different widgets. While a drag is under
way the handle in hand SHALL stay lit, wherever the pointer has since
travelled.

#### Scenario: An arrow lights under the pointer
- **WHEN** the pointer rests over a manipulator arrow
- **THEN** that arrow is drawn differently from the rest of the widget

#### Scenario: A drag keeps its handle lit
- **WHEN** a handle is being dragged and the pointer travels off it
- **THEN** the handle in hand stays lit

### Requirement: A cage owns the pointer, cursor and all
While a deformation cage is up, the brush cursor SHALL NOT be drawn over the
form. A ring under the pointer states that the next press leaves a stroke, and
while a cage is up no press does.

The rule SHALL be one rule covering every mode that takes the press away from
the brush — the whole-subtool manipulator and the cage alike.

#### Scenario: No brush is drawn while a cage is up
- **WHEN** a deformation cage is up and the pointer is over the form
- **THEN** no brush cursor is drawn, and a press leaves no stroke

### Requirement: A press that takes hold of nothing gathers control points
While a deformation cage is up, a primary press that grabs neither a
manipulator handle nor a control point SHALL draw a selection box across the
viewport, and on release every control point inside the box SHALL become the
selection — including points standing behind the form, which the viewport
already draws through for that reason.

With the add modifier held, the box's catch SHALL be added to the selection
rather than replacing it. A press and release in one place SHALL be a click on
nothing, which clears the selection; a small movement between them SHALL NOT
turn it into a box.

The selection SHALL be resolved when the pointer is released rather than while
the box is drawn, so the manipulator does not wander to the middle of whatever
is momentarily enclosed.

Turning the camera SHALL remain available while a cage is up, on the secondary
button and under the orbit modifier.

#### Scenario: A box takes a whole face at once
- **WHEN** the user drags a box around one face of the cage
- **THEN** every control point on that face is selected, front and back, and
  the manipulator stands on the middle of them

#### Scenario: A box adds to what is held
- **WHEN** the user drags a box with the add modifier held
- **THEN** the points it catches are added to the selection already held

#### Scenario: A click on nothing puts the manipulator away
- **WHEN** the user presses and releases on empty space without dragging
- **THEN** the selection is cleared and no manipulator is drawn

#### Scenario: The cage can still be looked at from behind
- **WHEN** the user drags with the secondary button, or with the orbit modifier
  held
- **THEN** the camera orbits and no selection box is drawn
