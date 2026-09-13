## MODIFIED Requirements

### Requirement: A released stroke settles from the brick cache
The viewport SHALL rebuild the settled surface from the document's bricks rather
than meshing the whole field.

A whole-field mesh has no region to test against, so the deformer cull cannot
fire and it pays for every grab on the layer; its cost therefore tracks the
**document** rather than the edit. It also cannot be patched incrementally,
which forces the surface to be laid out again in full and forces the next
incremental sync to discard it and rebuild from bricks regardless — so the field
is meshed twice for one stroke.

The viewport MAY mesh the whole field only where a defect in the brick mesher
makes its output unusable, and SHALL remove that route when the defect is fixed
rather than keeping it beside the cheaper one.

#### Scenario: A stroke is released
- **WHEN** a gesture ends and the surface settles
- **THEN** the surface is rebuilt per brick, and no whole-document mesh is taken

#### Scenario: The field is emptied
- **WHEN** the last field content is removed
- **THEN** the drawn surface goes with it, rather than the old surface standing
  in the buffers because a whole-field mesh refused an empty document

## ADDED Requirements

### Requirement: The coarse surface is shaded from the field
Where the engine can answer gradient normals at a level, the viewport SHALL ask
for them when drawing the coarse surface rather than falling back to face
normals.

Face normals on a coarse lattice measure up to 84.78 degrees away from the
field, which makes a coarse draw a visible downgrade rather than a cheaper route
to the same picture.

The document SHALL be supplied wherever gradients are asked for, since a
gradient is sampled through it. A live gesture is the exception and for a
different reason: the preview's lattice is not the document's field, so
attributing gradient normals through the document would shade the previewed
surface with the one it is standing in for.

#### Scenario: The camera drops to the coarse surface
- **WHEN** the coarse level is drawn outside a live gesture
- **THEN** its normals come from the field rather than from the triangles
