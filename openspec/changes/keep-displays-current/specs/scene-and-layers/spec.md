## MODIFIED Requirements

### Requirement: Geometry statistics are displayed for the current document
The application SHALL display the current polygon, vertex and triangle counts and the object count for the document, updating after edits that change them.

The counts SHALL be re-read whenever the viewport rebuilds what they count —
after the re-mesh an edit asked for, and after a carried layer's geometry is
rebuilt without one — so they never show the previous update's values.

#### Scenario: Counts follow edits
- **WHEN** an edit changes the meshed geometry
- **THEN** the displayed counts update to the new values

#### Scenario: Counts follow a rebuild that moves no brick
- **WHEN** a mesh layer is remeshed, or a layer's display changes, and the
  viewport rebuilds its geometry
- **THEN** the displayed counts are the rebuilt geometry's, without waiting for
  another edit

#### Scenario: Counts describe what is displayed
- **WHEN** counts are shown alongside a viewport displaying a reduced level of detail
- **THEN** the counts state which resolution they describe, so a reduced LOD is not read as a smaller model
