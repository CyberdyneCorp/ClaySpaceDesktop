## ADDED Requirements

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
