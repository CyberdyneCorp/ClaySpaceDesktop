## Purpose

A placed object: a primitive or a converted custom shape that sits in a layer,
can be pointed at, and combines with what is under it — the operand a boolean
needs and the thing a manipulator grabs.

## ADDED Requirements

### Requirement: A primitive can be placed in the scene
The application SHALL offer the engine's bounded primitives and place the
chosen one into the active SDF layer as an object, at a stated position and
size, selected on arrival.

An unbounded primitive SHALL NOT be offered. The engine names two — a plane and
an infinite cylinder — and neither has an extent to draw a manipulator around
or bounds for the brick cache to work from, so offering one would be offering a
control whose result cannot be shown.

#### Scenario: A cylinder is placed
- **WHEN** the user chooses the cylinder and places it
- **THEN** the layer holds a cylinder at the chosen position, it is the current
  selection, and the viewport shows it combined with the surface

#### Scenario: The list is what the engine can bound
- **WHEN** the primitive list is presented
- **THEN** it holds only primitives with a finite extent

#### Scenario: Placing into a layer that cannot take one
- **WHEN** the active layer is a voxel grid or a mesh
- **THEN** placing is refused with a reason naming what an object needs

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
