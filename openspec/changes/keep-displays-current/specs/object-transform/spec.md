## ADDED Requirements

### Requirement: An object is shown and manipulated where it stands
An object's position, rotation and scale SHALL be reported, drawn and
manipulated in the world, composed with the placement of the subtool that holds
it. Its outline, its manipulator and its readout SHALL follow the subtool when
the subtool is moved, turned or stretched. A transform given for an object SHALL
be taken in the world, and a placement aimed at a world point SHALL land there
inside a moved subtool.

Writing back a reported transform unchanged SHALL move nothing.

#### Scenario: A moved subtool carries its objects' outline
- **WHEN** an object is selected and its subtool is moved or stretched
- **THEN** the object's outline, manipulator and readout stand where the object
  now is, and none is left where it stood

#### Scenario: A placement lands where it was aimed
- **WHEN** a shape is placed into a subtool that has been moved
- **THEN** it stands at the point it was aimed at
