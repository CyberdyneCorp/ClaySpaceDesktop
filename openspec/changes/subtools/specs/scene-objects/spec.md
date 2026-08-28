## MODIFIED Requirements

### Requirement: A primitive can be placed in the scene
The application SHALL offer the engine's bounded primitives and place the
chosen one either as a new subtool of its own or into the active SDF layer as
an object, at a stated position and size, selected on arrival. Which of the
two happens SHALL be the sculptor's choice, and inserting as a new subtool
SHALL be the default.

A form put into the scene to be worked on its own is a subtool; a form put
into the layer being worked is a part of that form. Both are wanted, and
guessing between them from context would be wrong half the time.

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

#### Scenario: The list is what the engine can bound
- **WHEN** the primitive list is presented
- **THEN** it holds only primitives with a finite extent

#### Scenario: Placing into a layer that cannot take one
- **WHEN** the active layer is a voxel grid or a mesh and the sculptor places
  into it
- **THEN** placing is refused with a reason naming what an object needs, while
  inserting the same primitive as its own subtool remains available

## ADDED Requirements

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
