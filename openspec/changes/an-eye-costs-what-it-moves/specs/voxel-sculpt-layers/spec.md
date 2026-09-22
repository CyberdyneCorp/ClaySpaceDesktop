## MODIFIED Requirements

### Requirement: A voxel layer can be drawn as boxes or as a smooth surface
The application SHALL let a sculptor choose whether a voxel layer is drawn as
the boxes it is or as a smooth surface over the same cells, and SHALL treat
that choice as a display setting: it changes no cell, records no history entry
and does not mark the document modified.

Where the smooth surface is drawn, the application SHALL supply the vertex
normals the engine's mesher does not carry.

The filtering that smooths further SHALL default to none, and the interface
SHALL say where a setting can delete detail.

The smooth surface SHALL be built only for grids that are drawn. A whole-grid
smooth mesh is the most expensive thing in the settle, and a hidden grid is in
no frame, so a display change SHALL NOT rebuild one. The work is deferred
rather than dropped: a grid that is shown again SHALL be drawn with the picture
that is chosen, and SHALL show the same surface a grid that was never hidden
would.

#### Scenario: The two pictures are different surfaces over the same cells
- **WHEN** the smooth picture is chosen
- **THEN** the drawn surface differs from the boxes
- **AND** the grid holds the same cells as before
- **AND** the history is unchanged

#### Scenario: The smooth surface is shaded
- **WHEN** the smooth picture is drawn
- **THEN** every vertex carries a normal
- **AND** they do not all point the same way

#### Scenario: Filtering says what it costs
- **WHEN** the blur is raised above zero
- **THEN** the interface says it deletes isolated voxels and thin detail

#### Scenario: A display change does not re-mesh a hidden grid
- **WHEN** the picture is changed while a grid is hidden
- **THEN** no smooth surface is built for that grid
- **AND** showing it again draws the chosen picture, the same surface a grid
  that was never hidden would have
