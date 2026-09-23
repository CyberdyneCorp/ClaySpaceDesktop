# voxel-sculpt-layers Specification

## Purpose
Recording a run of voxel strokes as an addressable layer whose strength stays
adjustable afterwards — so a pass of detail can be dialled back hours later
rather than only undone at the time.

## Requirements

### Requirement: A run of voxel strokes can be recorded as a sculpt layer
The application SHALL allow the user to begin and end recording on a voxel
layer, and SHALL record what the strokes between those points changed as a named
sculpt layer.

#### Scenario: Recording captures a run
- **WHEN** the user begins recording, makes several strokes, and ends recording
- **THEN** a sculpt layer holding those strokes' changes exists on the grid

#### Scenario: Recording state is visible
- **WHEN** recording is in progress
- **THEN** the application shows that it is, so a sculptor cannot record
  unknowingly

### Requirement: A sculpt layer's strength is adjustable after the fact
The application SHALL allow a recorded sculpt layer's strength to be changed
after it was recorded, and SHALL show the result on the surface.

#### Scenario: Dialling a finished pass back
- **WHEN** the user reduces the strength of a sculpt layer recorded earlier
- **THEN** the surface shows less of that pass, without the strokes being redone

#### Scenario: Strength is not undo
- **WHEN** the user changes a sculpt layer's strength and then undoes
- **THEN** the strength change is undone, and the sculpt layer still exists

### Requirement: Sculpt layers are presented as a stack
The application SHALL present a voxel layer's sculpt layers as an ordered stack
that can be shown, hidden, reordered, merged down and removed, and SHALL report
what each costs in memory.

Showing, hiding and reordering are each one step of the history, as changing a
strength is. Merging a pass down and removing one **discard the recorded diff
they act on**, which nothing can put back, so the application SHALL NOT offer
them as something to take back and SHALL NOT leave a way back into that stack
that a later step could apply to a renumbered one.

Every operation on the stack SHALL state its refusal — a second recording
opened over an open one, a merge with nothing beneath it — on the line the
interface already shows scene refusals on.

#### Scenario: A sculpt layer is hidden
- **WHEN** the user hides a sculpt layer
- **THEN** the surface shows the sculpt without that layer's contribution

#### Scenario: Hiding a pass is one thing to take back
- **WHEN** the user hides a sculpt layer and undoes
- **THEN** the pass is shown again and nothing else moved

#### Scenario: Cost is reported
- **WHEN** the sculpt layer stack is shown
- **THEN** each layer's memory cost, and the total, are stated

#### Scenario: A refused operation on the stack is stated
- **WHEN** the user asks for something the stack cannot do
- **THEN** the interface says why, rather than the request appearing to have
  worked

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
