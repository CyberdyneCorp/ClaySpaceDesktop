## MODIFIED Requirements

### Requirement: The engine's combine operations and blend profiles are selectable
The application SHALL let a sculptor choose the combine operation an SDF edit
uses from those the engine provides, and the blend profile it is applied under.

The same vocabulary SHALL serve both kinds of edit: a stroke, where the choice
is made before the gesture and holds for it, and a placed object, where the
choice is a property of the object and stays editable for as long as the object
does. An operation SHALL mean the same thing in both.

This replaces a rule that spoke only of the edit about to be made. The
operations were always the engine's, and always applied to an item — a stroke's
item is created and left behind, and an object's stays addressable. Naming only
the first left the fourteen operations reachable exclusively through a gesture,
which is not how a boolean is used.

#### Scenario: An operation is chosen
- **WHEN** the user chooses a combine operation before making an edit
- **THEN** the edit is recorded with that operation

#### Scenario: A blend profile is chosen
- **WHEN** the user chooses a blend profile
- **THEN** edits made under it use that profile

#### Scenario: An object is placed with an operation
- **WHEN** the user places an object having chosen subtract
- **THEN** the object subtracts from what is under it, and the choice is
  recorded on the object rather than consumed
