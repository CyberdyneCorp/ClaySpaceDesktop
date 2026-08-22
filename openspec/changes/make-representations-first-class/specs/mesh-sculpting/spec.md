## Purpose

Sculpting an imported mesh layer's own vertices — the return trip that lets a
retopologized model be refined in place — under the guarantee that its topology
is never changed.

## ADDED Requirements

### Requirement: A mesh layer's vertices are sculptable
The application SHALL allow a mesh layer to be sculpted with the engine's
fixed-topology brushes. Each offered brush SHALL map to a documented engine
verb, as every other tool in the application does.

#### Scenario: A stroke moves a mesh layer's vertices
- **WHEN** the user strokes across an active mesh layer with a mesh brush
- **THEN** the mesh's vertices move and the viewport shows the result

#### Scenario: A mesh layer is pickable
- **WHEN** the pointer is over a mesh layer's surface
- **THEN** the brush cursor sits on that surface, and a press begins a stroke
  rather than orbiting

### Requirement: Sculpting a mesh never changes its topology
The application SHALL NOT create, split or delete a polygon while sculpting a
mesh layer. A mesh exported after sculpting SHALL carry the same indices, and
the same quads where it had them, as before.

#### Scenario: Indices survive a stroke
- **WHEN** a mesh layer is sculpted and then exported
- **THEN** its face indices are unchanged from before the stroke

#### Scenario: Quads survive a stroke
- **WHEN** a mesh layer imported with quads is sculpted and exported
- **THEN** its quads are unchanged from before the stroke

### Requirement: Stretching is shown rather than prevented
The application SHALL report when sculpting has stretched a mesh's triangles
beyond a stated quality, so that a sculptor learns the mesh wants retopology
rather than discovering it at export.

#### Scenario: A heavy pull is reported
- **WHEN** a stroke stretches the mesh past the stated quality
- **THEN** the application reports the quality and names retopology as the
  remedy

### Requirement: A mesh layer's colour is editable where it has colour
The application SHALL offer the colour brushes on a mesh layer that carries a
colour attribute, and SHALL refuse them with a stated reason on one that does
not, rather than creating the attribute silently.

#### Scenario: Painting a coloured mesh
- **WHEN** the user paints on a mesh layer carrying colour
- **THEN** the vertex colours change and no vertex moves

#### Scenario: A mesh with no colour refuses
- **WHEN** the user selects a colour brush on a mesh layer with no colour
  attribute
- **THEN** the brush is unavailable and states that the mesh carries no colour

### Requirement: Mesh deformers act on the whole form
The application SHALL offer taper, twist and a lattice cage on a mesh layer as
operations on the form rather than as brushes, with no centre, radius or
falloff.

#### Scenario: A taper reaches the whole layer
- **WHEN** the user tapers a mesh layer
- **THEN** every vertex is mapped, without a brush position being needed

#### Scenario: A lattice cage moves the form
- **WHEN** the user moves a lattice control point over a mesh layer
- **THEN** the form follows the cage

### Requirement: A mesh gesture is one undo step
The application SHALL record a mesh sculpting gesture as a single undoable
action that reverts the mesh exactly.

#### Scenario: One gesture, one undo
- **WHEN** the user completes a mesh stroke and undoes
- **THEN** the mesh is exactly as it was before the stroke began
