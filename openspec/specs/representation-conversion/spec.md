# representation-conversion Specification

## Purpose
Moving a layer between SDF, voxel and mesh so a sculptor can use each
representation for what it is good at, and knowing what each crossing costs
before paying it.
## Requirements
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

Every crossing that samples onto a lattice SHALL state those losses, and the
crossing from a mesh into a field is one of them. It samples the model onto a
lattice like the others; what differs is only that the resolution was chosen
for the sculptor rather than by them, which is a reason to offer the choice and
not a reason to price the crossing at nothing.

This replaces a rule that was true of what the application *displayed* and not
of what it did. Mesh-to-SDF was excluded from the directions that choose a
resolution, so a sculptor crossing an imported model into clay was told the
surface would not move, nothing would be lost, and its sharp edges would
survive — none of which is true of a lattice sampling. Found by placing a mesh
as a boolean operand, which pays the same crossing and was made to state the
same costs, and stated none.

#### Scenario: The cost is shown before committing
- **WHEN** the user opens the conversion for an SDF layer
- **THEN** the losses for that direction are stated, and the conversion has not
  yet run

#### Scenario: The cost reflects the chosen resolution
- **WHEN** the user changes the cell size in the conversion
- **THEN** the stated surface movement changes with it

#### Scenario: A mesh crossing into a field is priced like the others
- **WHEN** the losses are stated for a crossing from a mesh into a field
- **THEN** they name the surface movement, the vanishing feature size and the
  loss of sharp edges, on the same terms as a crossing into a grid

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

### Requirement: A mesh offered as a boolean operand is converted on use
Where a sculptor uses a mesh layer as the operand of a boolean, the application
SHALL offer to convert it rather than refusing, SHALL state the same costs the
conversion panel states before it runs, and SHALL leave the source mesh layer
where it is.

A mesh cannot compose: it is not an operand of a boolean belonging to another
layer until it is crossed to a field, and paying that crossing quantises the
vertices and drops the edge loops that made it worth keeping as a mesh. That
remains true. What changes is that the sculptor meets it as an offer with a
price on it, at the moment they are trying to use the mesh, rather than as a
refusal pointing at a panel elsewhere.

#### Scenario: A custom object is subtracted
- **WHEN** the user chooses an imported mesh as the shape to subtract
- **THEN** the crossing's costs are stated, and on accepting them a converted
  copy becomes the operand while the mesh layer stays as it was

#### Scenario: The costs are the panel's own
- **WHEN** the costs are stated for a conversion on use
- **THEN** they are the same figures the conversion panel computes for that
  crossing at that resolution

#### Scenario: Declining leaves everything alone
- **WHEN** the user declines the conversion
- **THEN** no layer is added, the mesh is unchanged, and no boolean is made

### Requirement: A mesh can be taken as a subdivision cage, and a level baked back
The application SHALL offer a crossing from a mesh layer to a subdivision
hierarchy, which takes the mesh as the hierarchy's cage, and a crossing from a
hierarchy to a mesh, which bakes a level. A hierarchy SHALL be reachable by no
other route, because those are the two calls the engine offers.

#### Scenario: A mesh becomes a cage
- **WHEN** the user crosses a mesh layer to a hierarchy
- **THEN** a hierarchy is added whose level zero is that mesh's own vertices

#### Scenario: A level becomes a mesh
- **WHEN** the user crosses a hierarchy to a mesh
- **THEN** a mesh layer is added holding the level's surface

#### Scenario: No other crossing reaches a hierarchy
- **WHEN** the crossings out of a field layer or a grid layer are listed
- **THEN** none of them ends in a hierarchy

### Requirement: The two hierarchy crossings state no cell-sized loss
The application SHALL NOT state a surface movement, a vanishing feature size or
a cell count for a crossing that samples nothing. A crossing that copies
vertices SHALL be reported as exact.

#### Scenario: A cage crossing moves no surface
- **WHEN** the cost of the crossing from a mesh to a hierarchy is shown
- **THEN** the surface movement and the vanishing feature size are zero, no
  cell size is chosen, and sharp edges are kept

#### Scenario: Baking a level moves no surface either
- **WHEN** the cost of the crossing from a hierarchy to a mesh is shown
- **THEN** the same is true of it

#### Scenario: Baking a level ends what stands under it
- **WHEN** the cost of the crossing from a hierarchy to a mesh is shown
- **THEN** it states that what stands behind the surface does not survive, as
  every other crossing states of the procedural history

### Requirement: A mesh that cannot be a cage is refused by the fault
The application SHALL refuse a crossing into a hierarchy by naming what is wrong
with the mesh — an edge shared by more than two faces, or a face with repeated
or collinear corners — rather than reporting only that the crossing failed. It
SHALL NOT repair the mesh, because a cage's topology is work somebody paid for.

#### Scenario: A non-manifold mesh is refused by name
- **WHEN** a mesh with an edge shared by more than two faces is crossed to a
  hierarchy
- **THEN** the refusal names the non-manifold edge

#### Scenario: A degenerate face is refused by name
- **WHEN** a mesh with a face with repeated or collinear corners is crossed
- **THEN** the refusal names the degenerate face

#### Scenario: The two refusals read differently
- **WHEN** the refusals for the two faults are compared
- **THEN** they are different sentences

### Requirement: Subdividing is priced on what the build holds at its worst
The application SHALL state, before a level is added, how many vertices and
faces it would create, what it would hold afterwards, and the high-water mark
during the build. It SHALL refuse a level on the high-water mark rather than on
what remains, and SHALL refuse one past the depth the engine takes.

#### Scenario: A level that fits once built and not while building is refused
- **WHEN** a level whose peak allocation exceeds the budget but whose persistent
  cost does not is priced
- **THEN** it is refused, and the refusal names the peak and the budget

#### Scenario: The depth ceiling is a refusal rather than a failure
- **WHEN** a hierarchy already as deep as the engine takes is asked for another
  level
- **THEN** it is refused, and the refusal names how deep it already is

#### Scenario: A face count that would overflow does not report as affordable
- **WHEN** the faces a great many further subdivisions would produce are
  projected
- **THEN** the projection saturates rather than wrapping to a small number

