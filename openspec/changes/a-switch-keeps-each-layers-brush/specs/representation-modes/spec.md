## ADDED Requirements

### Requirement: A substitute tool is chosen from the capability table
When the active layer changes to a representation that has no verb for the
active tool, the application SHALL choose the substitute from the same
capability table the shelf reads, and SHALL NOT keep a separate list of
substitutes.

The substitute SHALL be, in order: a tool offered there whose binding carries
the same semantic intent and is the representation's native verb for it; else a
tool offered there with the same intent at any fidelity; else the first tool the
representation's shelf lists. A tool driven by a different gesture, or one that
refuses the layer until a pass is selected, SHALL NOT be chosen as a substitute.
The choice SHALL depend on the tool and the representation alone.

While a substitute is in hand, the application SHALL remember the tool that was
chosen: a later switch to a layer that carries the chosen tool SHALL return it,
and choosing any tool SHALL end the substitution.

#### Scenario: The same act is preferred
- **WHEN** Raspar is active on a voxel layer and an SDF layer is made active
- **THEN** Planar, the field's flatten, is the active tool, and the status line
  says the tool changed

#### Scenario: A tool with no counterpart falls to the shelf's first
- **WHEN** a tool whose act the new representation has no tool for is active
  and a layer of that representation is made active
- **THEN** Padrão is the active tool

#### Scenario: A different gesture is not a substitute
- **WHEN** Trim is active on an SDF layer and a voxel layer is made active
- **THEN** the active tool is not the grid's eraser, although both remove
  material

#### Scenario: The chosen tool comes back
- **WHEN** a switch replaced the chosen tool and the user switches to a layer
  whose representation carries it
- **THEN** the chosen tool is active again, with the settings it had there

### Requirement: Each representation has a documented default brush
A tool used for the first time on a representation SHALL start from that
representation's documented default settings, and SHALL NOT start from a value
set on another representation.

#### Scenario: A large field brush does not reach a new grid
- **WHEN** a brush is set to a large size on an SDF layer and a voxel layer is
  added
- **THEN** the options bar and the first stroke on the voxel layer both use the
  voxel default size
