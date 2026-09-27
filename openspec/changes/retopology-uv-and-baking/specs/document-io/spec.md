## ADDED Requirements

### Requirement: An export says when it dropped a layer's UVs
The application SHALL tell the sculptor when a visible mesh layer carried UV
coordinates, the chosen format stores them, and the file it wrote carries none.

The engine combines an export by concatenation, and an attribute any input
lacks is dropped from all of them. The meshed field never carries UVs, so a
retopology's layout exported beside a visible sculpt is lost — and without
this finding the file looks like a success.

#### Scenario: A layout is exported beside the sculpt it was made from
- **WHEN** a mesh layer carrying UVs is exported to OBJ while the sculpt it was
  retopologised from is also visible
- **THEN** the sculptor is told the layer's UVs were dropped and that hiding the
  other layers exports them

### Requirement: A document of visible mesh layers alone can be exported
An export SHALL succeed when the only visible geometry is mesh layers. The
engine's combined export meshes the field first and refuses an empty one; the
application SHALL then write the visible mesh layers alone, each placed by its
layer transform, concatenated under the engine's own attribute rule.

#### Scenario: A retopology is exported on its own
- **WHEN** every layer but a retopology result carrying UVs is hidden and the
  document is exported to OBJ
- **THEN** the file is written, holds that mesh, and carries its UVs
