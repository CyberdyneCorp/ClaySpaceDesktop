# scene-objects Specification

## Purpose
A placed object: a primitive or a converted custom shape that sits in a layer,
can be pointed at, and combines with what is under it — the operand a boolean
needs and the thing a manipulator grabs.

## Requirements

### Requirement: A primitive can be placed in the scene
The application SHALL offer the engine's bounded primitives and place the
chosen one either as a new subtool of its own or into the active SDF layer as
an object, at a stated position and size, selected on arrival. Which of the
two happens SHALL be the sculptor's choice, and inserting as a new subtool
SHALL be the default.

A form put into the scene to be worked on its own is a subtool; a form put
into the layer being worked is a part of that form. Both are wanted, and
guessing between them from context would be wrong half the time.

The shapes SHALL be offered in a section of the right region rather than in a
window over the viewport, because the viewport holds the surface the shape is
placed onto; the section SHALL open from the tool rail's shapes button or its
menu entry and SHALL close from a control on its own heading, each dispatching
the same command.

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
- **WHEN** the user chooses the cylinder and places it into the active layer
- **THEN** the layer holds a cylinder at the chosen position, it is the current
  selection, and the viewport shows it combined with the surface

#### Scenario: A sphere arrives as its own subtool
- **WHEN** the user inserts a sphere as a subtool
- **THEN** a new subtool holds the sphere, it is the active subtool, and
  sculpting lands on it rather than on the form that was active before

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
- **WHEN** the active layer is a voxel grid or a mesh and the sculptor places
  into it
- **THEN** placing is refused with a reason naming what an object needs, while
  inserting the same primitive as its own subtool remains available

### Requirement: A placed object remains addressable
An object SHALL stay in the document as an addressable item for as long as the
layer holds it. Selecting it, transforming it, changing what it combines with
and removing it SHALL all remain available after the sculptor has gone on to do
something else, and SHALL survive saving and reopening the document.

This is what makes it an operand rather than a stamp: the boolean is
re-evaluated from the object's current state, so moving it moves the hole.

#### Scenario: A boolean follows its operand
- **WHEN** a subtracted cylinder is moved after other edits have been made
- **THEN** the cavity is where the cylinder now is, and the later edits are
  still there

#### Scenario: An object survives a reopen
- **WHEN** a document holding a placed object is saved and reopened
- **THEN** the object is still selectable and still carries its transform and
  its operation

### Requirement: An object carries its own combine operation and blend profile
An object SHALL carry the combine operation and blend profile it was placed
with, and both SHALL be editable afterwards without replacing the object or
losing its transform.

The three booleans — union, subtraction, intersection — SHALL be offered
directly as a row of controls carrying their two-disc marks, ahead of the full
list of operations, in the interface's language.

#### Scenario: A subtraction is chosen without opening the list
- **WHEN** a placed object is selected
- **THEN** union, subtraction and intersection are each one click away, and
  the chosen one reads as chosen

#### Scenario: An operation is changed after placement
- **WHEN** a subtracted object is changed to a groove
- **THEN** the surface shows a groove where the subtraction was, and the object
  has not moved

#### Scenario: An operation that needs a distance
- **WHEN** an object is given an operation that does nothing at zero distance
- **THEN** the distance control cannot reach zero, on the same terms a stroke's
  does

### Requirement: An object's primitive can be exchanged
The application SHALL let a sculptor change which primitive an object is
without losing its transform, its operation, its blend or its place in the
layer's order.

#### Scenario: A box becomes a cylinder
- **WHEN** a placed box is changed to a cylinder
- **THEN** a cylinder stands where the box stood, subtracting as the box did

### Requirement: An object can be removed
The application SHALL let a sculptor remove a placed object, and the surface
SHALL return to what it was without it.

#### Scenario: Removing a subtraction closes the hole
- **WHEN** a subtracted object is removed
- **THEN** the cavity it cut is gone and the rest of the form is unchanged

### Requirement: Placing, changing and removing an object are each one undo step
Each of placing an object, transforming it, changing its primitive or its
operation, and removing it SHALL be a single entry in the undo history.

#### Scenario: One undo takes back one placement
- **WHEN** an object is placed and the user undoes once
- **THEN** the object is gone and nothing else has changed

#### Scenario: A drag is one step, not one per frame
- **WHEN** an object is dragged across the form and the user undoes once
- **THEN** the object returns to where the drag began

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

### Requirement: An imported or existing form can be inserted as a subtool
The application SHALL let a sculptor bring a form into the scene as a subtool
from three sources: the bounded primitives, a mesh imported from a file, and a
copy of a subtool already in the document. Each SHALL arrive selected, at a
stated position, and sculptable on the terms its representation allows.

A copy SHALL be independent: sculpting the copy SHALL NOT change the original.

#### Scenario: An imported mesh becomes a subtool
- **WHEN** the sculptor imports a mesh as a subtool
- **THEN** it stands in the scene as its own subtool, carries its geometry, and
  can be moved with the manipulator

#### Scenario: A copied subtool is independent
- **WHEN** a subtool is copied and the copy is sculpted
- **THEN** the original is unchanged, and both are present in the scene

#### Scenario: Insertion is one undo step
- **WHEN** a subtool is inserted from any of the three sources and the sculptor
  undoes once
- **THEN** the subtool is gone and nothing else has changed
