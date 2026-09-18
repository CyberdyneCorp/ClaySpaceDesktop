## Purpose

Moving a layer between SDF, voxel and mesh so a sculptor can use each
representation for what it is good at, and knowing what each crossing costs
before paying it.

## ADDED Requirements

### Requirement: A layer can be converted to another representation
The application SHALL offer conversion of the active layer to another
representation where the engine supports that direction: SDF to voxel, voxel to
SDF, mesh to voxel, and mesh to SDF.

#### Scenario: SDF becomes voxel
- **WHEN** the user converts an SDF layer to voxels over a region
- **THEN** a voxel layer holding the rasterized sculpt is added to the document

#### Scenario: Voxel becomes SDF
- **WHEN** the user converts a voxel layer to SDF
- **THEN** an SDF layer is added whose content evaluates as a distance field and
  can be used as a boolean operand

#### Scenario: A coloured voxel sculpt keeps its colour
- **WHEN** a voxel layer carrying more than one palette entry is converted to
  SDF
- **THEN** the resulting layer reproduces those colours

#### Scenario: Mesh becomes voxel directly
- **WHEN** the user converts a mesh layer to voxels
- **THEN** the grid is filled from the triangles themselves rather than by way
  of a distance field, and the mesh's vertex colours reach the palette

### Requirement: A conversion states its cost before it runs
The application SHALL state, before a conversion is performed, what that
conversion loses. The statement SHALL name the surface movement in terms of the
chosen cell size, the loss of features thinner than a cell, the loss of sharp
edges to a staircase, and the loss of the procedural edit history where the
direction discards it.

#### Scenario: The cost is shown before committing
- **WHEN** the user opens the conversion for an SDF layer
- **THEN** the losses for that direction are stated, and the conversion has not
  yet run

#### Scenario: The cost reflects the chosen resolution
- **WHEN** the user changes the cell size in the conversion
- **THEN** the stated surface movement changes with it

### Requirement: A conversion adds a layer, or replaces the one it read
The application SHALL produce a new layer from a conversion and SHALL leave the
source layer unchanged by default, so that a crossing can be reconsidered
without redoing the work that led to it.

The application SHALL also offer a conversion **in place**: the source layer
leaves as the result arrives, and the result takes the source's row in the
stack. The interface SHALL state which of the two a crossing will do before it
runs, and adding SHALL be the default, since it is the one that cannot lose
work.

A crossing SHALL be one undo step either way, and the depth the interface
reports SHALL count it as one. An in-place crossing leaves more than one engine
entry — the removal and the reorder are recorded separately and an undo group
does not swallow them — so the application SHALL record how many it left and
step over all of them together.

#### Scenario: The source survives a crossing that adds
- **WHEN** a conversion completes without being asked to replace
- **THEN** the source layer is still present with its content unchanged

#### Scenario: A crossing in place replaces the layer it read
- **WHEN** a conversion is run in place
- **THEN** the source layer is gone, the result stands in the row the source
  held, and the stack is no taller than before

#### Scenario: One undo takes a crossing back
- **WHEN** the user undoes once after a crossing
- **THEN** the document holds what it held before it, including the source
  layer where the crossing replaced one

### Requirement: A conversion that cannot succeed is refused with a reason
The application SHALL refuse a conversion it cannot perform — an unbounded
region, a resolution whose grid would exceed the memory budget, or a source
carrying nothing — and SHALL state which of those it was.

#### Scenario: An unbounded region is refused
- **WHEN** the user asks to rasterize a layer with no bounds and supplies no
  region
- **THEN** the conversion is refused and states that a region is required

#### Scenario: An unaffordable resolution is refused
- **WHEN** the chosen cell size would produce a grid beyond the memory budget
- **THEN** the conversion is refused and states the budget it would exceed
