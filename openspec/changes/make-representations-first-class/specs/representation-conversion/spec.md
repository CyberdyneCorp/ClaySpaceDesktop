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

### Requirement: A conversion adds a layer rather than replacing one
The application SHALL produce a new layer from a conversion and SHALL leave the
source layer unchanged, so that a crossing can be reconsidered without redoing
the work that led to it.

A conversion SHALL NOT be undoable, and the application SHALL say so where it
offers one: a crossing is taken back by removing the layer it added. The source
layer surviving is what makes that sufficient.

This is the engine's shape rather than a choice. A conversion produces no undo
entry — layer creation and rasterization are not recorded, and a voxel layer
carries no history at all by construction — so there is nothing for undo to
take back. Grouping the crossing's edits was tried and groups nothing. An
application-side history entry could remove the layer on undo, and would put a
second history beside the engine's for one operation, which is a larger claim
than the operation is worth: removing a layer is already one click and already
undoes nothing else.

#### Scenario: The source survives
- **WHEN** a conversion completes
- **THEN** the source layer is still present with its content unchanged

#### Scenario: A conversion is taken back by removing its layer
- **WHEN** the user removes the layer a conversion produced
- **THEN** the document holds what it held before the conversion

#### Scenario: The interface does not offer undo for a crossing
- **WHEN** the user is shown a conversion before running it
- **THEN** the interface states that the crossing is not undoable and that the
  source layer is what it is taken back with

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
