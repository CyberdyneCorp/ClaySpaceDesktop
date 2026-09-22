## ADDED Requirements

### Requirement: A form larger than the field can hold is refused before it is placed
The application SHALL price the region a placed form would fill against what
this document's brick cache can hold, and SHALL refuse a placement over that
budget with a sentence naming both the figure and the limit.

The price SHALL be paid before anything reaches the document, so that a refused
placement leaves no item, no history entry and no dirty region behind.

The same price SHALL be paid by every route that measures a form: placing one
into the active layer, inserting one as a subtool of its own, and re-measuring
one already placed. No route SHALL be a way round another.

The region SHALL be measured per axis rather than as a cube of the longest
side, so that a long thin form is still placeable.

#### Scenario: A form larger than the document
- **WHEN** a sculptor asks for a form whose box fills more of the field than the
  cache can hold
- **THEN** the placement is refused, the refusal names what it would fill and
  what the document holds, and the document is unchanged

#### Scenario: Each parameter is inside its bound and the form is not
- **WHEN** a torus is asked for with both radii at the largest a parameter
  allows, so its box reaches four times that
- **THEN** the placement is refused, rather than being accepted because each
  number was in range

#### Scenario: Re-measuring a placed form is priced too
- **WHEN** a form already in the layer is given sizes whose region the cache
  could not hold
- **THEN** the change is refused and the form keeps the sizes it had

#### Scenario: A long thin form is still placed
- **WHEN** a cylinder is asked for with a small radius and a large half-height
- **THEN** it is placed, because the region is priced on each axis

### Requirement: A clamped size is reported with the value used
Where a size a sculptor asked for is brought inside the bounds, the application
SHALL say so and name the value actually used, beside the answer rather than in
place of it.

The report SHALL NOT be a refusal: what was asked for did happen, at a
different number. A size the caller never supplied — a short list filled out
from the defaults, which is a document written by another version — SHALL NOT
be reported, because nobody asked for it.

#### Scenario: A size past the bound
- **WHEN** a sculptor sets a radius above the largest the field can hold
- **THEN** the control takes the largest it can, and says which number it used

#### Scenario: A size inside the bound
- **WHEN** a sculptor sets a radius the field can hold
- **THEN** nothing is said about it

## MODIFIED Requirements

### Requirement: A primitive can be placed in the scene
The application SHALL offer the engine's bounded primitives and place the
chosen one into the active SDF layer as an object, at a stated position and
size, selected on arrival.

The sizes offered SHALL be bounded by what the field can hold — derived from
the brick cache's own layout and budget — rather than by a fixed number. A size
the cache cannot carry SHALL NOT be offered.

The shapes SHALL be offered in a section of the right region rather than in a
window over the viewport, because the viewport holds the surface the shape is
placed onto; the section SHALL open from the tool rail's shapes button or its
menu entry and SHALL close from a control on its own heading, each dispatching
the same command.

An unbounded primitive SHALL NOT be offered. The engine names two — a plane and
an infinite cylinder — and neither has an extent to draw a manipulator around
or bounds for the brick cache to work from, so offering one would be offering a
control whose result cannot be shown.

#### Scenario: A cylinder is placed
- **WHEN** the user chooses the cylinder and places it
- **THEN** the layer holds a cylinder at the chosen position, it is the current
  selection, and the viewport shows it combined with the surface

#### Scenario: The picker stands beside the viewport
- **WHEN** the shapes section is open
- **THEN** it is drawn in the right region and the viewport is not covered by it

#### Scenario: The list is what the engine can bound
- **WHEN** the primitive list is presented
- **THEN** it holds only primitives with a finite extent

#### Scenario: The sizes offered are what the field holds
- **WHEN** a size control is offered for any of the shapes
- **THEN** its largest value is one the field can carry, taken from the cache's
  layout and budget rather than fixed

#### Scenario: Placing into a layer that cannot take one
- **WHEN** the active layer is a voxel grid or a mesh
- **THEN** placing is refused with a reason naming what an object needs
