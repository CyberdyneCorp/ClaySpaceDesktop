## ADDED Requirements

### Requirement: A mesh layer's topology can be rebuilt
The application SHALL let a sculptor rebuild a mesh layer's topology through a
voxel field, so that overlapping shells fuse, self-intersections resolve,
stretched triangles disappear and the density comes out uniform. This is the
repair for a form that has been pulled somewhere its triangles cannot follow,
and it is the mesh counterpart to collapsing a field layer that has steepened.

It SHALL be offered rather than taken: it destroys the topology it replaces, and
the decision is the sculptor's. Unlike the field layer's collapse — which is
offered only when the engine advises it — the rebuild SHALL be available
whenever a mesh layer is active. The engine measures a field's steepening and
can say when collapsing is worth it; there is no equivalent measurement for a
topology that has stopped taking detail, and the sculptor is the one who can see
that.

The sculptor SHALL choose the density, stated as cells across the form's longest
extent so that it means the same thing on any size of form, and the application
SHALL report back what that came to in world units.

The rebuild SHALL be one undoable step. A refusal SHALL leave the layer exactly
as it was, which is what makes it safe to offer a density the form may turn out
not to survive.

#### Scenario: A rebuilt layer holds new triangles
- **WHEN** the sculptor rebuilds an active mesh layer
- **THEN** the layer holds the rebuilt triangles and the viewport draws them

#### Scenario: The density is what was asked for
- **WHEN** the same form is rebuilt at two different densities
- **THEN** the coarser request produces fewer triangles and a larger cell

#### Scenario: A representation with no topology refuses by name
- **WHEN** a rebuild is asked of a field or a grid layer
- **THEN** it is refused with a reason naming what a rebuild applies to

#### Scenario: A rebuild is one step on the undo menu
- **WHEN** the sculptor undoes a rebuild
- **THEN** the triangles it replaced are back

### Requirement: A rebuild states what it destroyed
The application SHALL report, after a rebuild, what the rebuild cost: the
triangle counts before and after, how many separate pieces the form is now in,
and each thing the operation destroyed that a sculptor cannot see by looking at
the result.

Vertex and polygon identity are destroyed every time and texture coordinates are
dropped rather than reprojected — the engine will not pretend to carry a UV
layout across a seam, because a stretched layout looks like a preserved one. The
application SHALL say so rather than leaving it to be discovered later.

The report SHALL persist beside the control rather than appearing once. The
question a sculptor asks — "did those two actually join?" — is asked after
looking at the result, and the piece count is where it is answered.

#### Scenario: The counts are shown
- **WHEN** a rebuild completes
- **THEN** the triangle counts before and after are shown beside the control

#### Scenario: Dropped texture coordinates are stated
- **WHEN** a rebuild drops the source's texture coordinates
- **THEN** the application says so, as a fact about the rebuild rather than as a
  failure

#### Scenario: A form still in pieces says so
- **WHEN** a rebuild meant to fuse leaves more than one piece
- **THEN** the number of pieces is shown

### Requirement: Sculpting survives a rebuild and its undo
A rebuild replaces every vertex and every index in the layer. Any adjacency,
bounding volume or sculptor the application holds over that layer SHALL be
discarded when it happens, and a stroke made immediately afterwards SHALL land.

This SHALL hold in **both** directions of history. Undoing a rebuild replaces the
triangles again, and so does redoing it; the engine's geometry revision does not
move when history does, so the application SHALL keep its own account of where a
rebuild sits in the history rather than relying on that number alone.

#### Scenario: A stroke lands on the rebuilt mesh
- **WHEN** the sculptor rebuilds a mesh layer and immediately sculpts it
- **THEN** the stroke lands and moves the surface

#### Scenario: A stroke lands after the rebuild is undone
- **WHEN** the sculptor rebuilds a mesh layer, undoes the rebuild, and sculpts
- **THEN** the stroke lands and moves the surface

#### Scenario: A stroke lands after the rebuild is redone
- **WHEN** the sculptor undoes a rebuild, redoes it, and sculpts
- **THEN** the stroke lands and moves the surface
