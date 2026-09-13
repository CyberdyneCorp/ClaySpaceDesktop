## ADDED Requirements

### Requirement: An export says when the mesh it wrote is not sound
The application SHALL validate the mesh an export actually writes, and SHALL
tell the sculptor when it is not watertight or not 2-manifold.

This is the class of defect that breaks a slicer, a boolean engine or a stricter
importer while a viewport shows nothing wrong, so it cannot be left to be
discovered in the file.

The finding SHALL carry the counts and not only the fault. A handful of pinched
edges in a large mesh is usually a file worth shipping and thousands of them is
not, and a sculptor told only that the mesh "is not manifold" cannot tell which
they have.

A validator that fails SHALL leave the export silent rather than failing it. The
file is the sculptor's work, and withholding it because the checker broke is the
worse trade.

#### Scenario: A decimated export comes back pinched
- **WHEN** an export is written whose mesh carries an edge with more than two
  incident triangles
- **THEN** the sculptor is shown that it is not manifold, and how many edges

#### Scenario: A sound export says nothing about itself
- **WHEN** an export is written whose mesh is watertight and 2-manifold
- **THEN** no finding is raised, because a warning that appears on every export
  teaches a sculptor to ignore the panel

### Requirement: What is predicted and what is observed are kept apart
The application SHALL distinguish what it can say about an export **before** the
write — derived from the format and the settings — from what it can only say
**after** it, derived from the bytes.

The two SHALL NOT produce the same message, so that a sculptor reading the panel
can tell a property of their choices from an observation about their file.

#### Scenario: A prediction and a finding do not restate each other
- **WHEN** a mesher is chosen that is described in advance as not manifold, and
  the written mesh is then found not to be
- **THEN** the panel carries both, and they are not the same sentence
