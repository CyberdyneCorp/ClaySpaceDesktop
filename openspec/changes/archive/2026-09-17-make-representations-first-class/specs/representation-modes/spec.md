## Purpose

What the active layer's representation is, where a sculptor can see it, and how
the shell's tools and panels follow it — so that each of SDF, voxel and mesh
offers its own vocabulary rather than one list with entries greyed out.

## ADDED Requirements

### Requirement: The active representation is visible without inspection
The application SHALL show the active layer's representation in the viewport
chrome and in the layer stack, at all times and without the user opening a
panel. The three SHALL be distinguishable from each other by more than colour
alone.

#### Scenario: The representation is on screen
- **WHEN** a layer is active
- **THEN** its representation is named in the viewport chrome and beside the
  layer in the stack

#### Scenario: Switching layers changes what is shown
- **WHEN** the user makes a layer of a different representation active
- **THEN** the displayed representation changes to match, in both places

### Requirement: The tool shelf offers the verbs the active representation has
The application SHALL present, for the active layer, the tools that exist for
that representation. A tool that has no verb on the active representation SHALL
NOT be offered. Whether a tool exists for a representation SHALL be derived from
one declared table rather than from a rule written per tool.

#### Scenario: The shelf follows the active layer
- **WHEN** the user makes a voxel layer active and then a mesh layer active
- **THEN** the shelf's contents change to the verbs each representation has

#### Scenario: A tool with no verb here is absent
- **WHEN** a representation has no verb for a tool
- **THEN** the tool is not shown in the shelf for that layer

#### Scenario: The active tool survives where it can
- **WHEN** the active tool exists on the newly active layer's representation
- **THEN** it stays active rather than being reset

#### Scenario: The active tool is replaced where it cannot
- **WHEN** the active tool does not exist on the newly active layer's
  representation
- **THEN** the application selects one that does and states that it changed

### Requirement: A tool unavailable for a reason other than its representation says so
The application SHALL continue to disable, with a stated reason, a tool that
exists for the active representation but cannot be used right now — a protected
layer, a hidden layer, or a missing prerequisite such as a mesh without a colour
attribute.

#### Scenario: A protected layer disables its tools
- **WHEN** the active layer is protected and a tool that exists for its
  representation is offered
- **THEN** the tool is disabled and names the protection as the reason

#### Scenario: A missing prerequisite is named
- **WHEN** a colour brush is offered on a mesh layer carrying no colour
  attribute
- **THEN** the tool is disabled and states that the mesh has no colour to paint

### Requirement: Brush settings are held per tool and per representation
The application SHALL remember brush settings for each tool separately on each
representation, so that returning to a tool on a layer returns the settings it
had there.

#### Scenario: Settings return with the layer
- **WHEN** the user sets a size on a tool on a voxel layer, works on an SDF
  layer, and returns to the voxel layer
- **THEN** the tool has the size it had on the voxel layer

### Requirement: The manipulator turns a selection in the screen's frame as well as the world's
The application SHALL offer, while the manipulator is set to rotate, a ring that
turns the selection about the axis facing the camera, in addition to the three
world-axis rings. The axis SHALL be fixed when the drag begins, so that moving
the camera during a drag does not alter the rotation.

#### Scenario: The outer ring turns in the screen plane
- **WHEN** the user drags the outer ring
- **THEN** the selection turns about the direction from it to the camera, and
  points along that direction do not move

#### Scenario: Moving the camera mid-drag does not twist the selection
- **WHEN** the camera moves while an outer-ring drag is in progress
- **THEN** the rotation continues about the axis the drag began with

### Requirement: A rotation can be snapped to whole increments
The application SHALL round a rotation to increments of 15 degrees while a
modifier is held, and SHALL read that modifier for as long as the drag lasts
rather than only when it begins. Snapping SHALL apply to rotation alone.

#### Scenario: A snapped turn lands on an increment
- **WHEN** the user turns the manipulator by an angle that is not a multiple of
  15 degrees, with the modifier held
- **THEN** the selection is turned by the nearest multiple of 15 degrees

#### Scenario: The modifier can be taken up part-way through a turn
- **WHEN** the user begins a turn without the modifier and presses it before
  releasing
- **THEN** the turn snaps from that point on

#### Scenario: Moving and scaling are unaffected
- **WHEN** the modifier is held during a move or a scale
- **THEN** the result is the same as without it
