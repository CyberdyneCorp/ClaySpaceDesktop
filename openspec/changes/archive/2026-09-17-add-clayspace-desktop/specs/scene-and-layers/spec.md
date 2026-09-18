## ADDED Requirements

### Requirement: The scene is presented as a navigable tree
The application SHALL present the document's objects and groups as a tree reflecting the engine's node structure, showing each entry's name and visibility, and allowing entries to be expanded, collapsed and selected.

#### Scenario: The tree reflects the document
- **WHEN** a group is created or removed through any path
- **THEN** the tree shows the change without requiring the user to refresh it

#### Scenario: Selecting in the tree selects in the viewport
- **WHEN** the user selects an entry in the scene tree
- **THEN** the corresponding geometry is indicated as selected in the viewport

### Requirement: Layers are presented as an ordered stack
The application SHALL present layers as an ordered stack, showing each layer's name, visibility, protection state and intensity, with the evaluation order matching the engine's ordered edit list. The user SHALL be able to create, rename, reorder and remove layers.

#### Scenario: Reordering changes evaluation order
- **WHEN** the user moves a layer above another
- **THEN** the document is re-evaluated with the new order and the viewport reflects the result

#### Scenario: Removing a layer is undoable
- **WHEN** the user removes a layer and undoes the removal
- **THEN** the layer returns with its content, position in the stack, and settings intact

### Requirement: Layer protection states are distinct and enforced
The application SHALL expose the engine's three protection states: visible, ghosted (shown, not pickable, not editable) and locked (shown, pickable, not editable). Attempting to edit a protected layer SHALL be refused with a stated reason rather than silently ignored.

#### Scenario: A ghosted layer is not picked
- **WHEN** the user clicks on geometry belonging to a ghosted layer
- **THEN** the pick passes through to whatever is behind it, and the ghosted layer is not selected

#### Scenario: Editing a locked layer is refused with a reason
- **WHEN** the user applies a brush to a locked layer
- **THEN** no edit occurs and the interface states that the layer is locked

### Requirement: Selection is driven by engine picking
Clicking in the viewport SHALL select through the engine's attributed raycast, resolving to the layer and item under the pointer, honoring ghost and lock states. Selection SHALL be reflected consistently in the viewport, the scene tree and the layer stack.

#### Scenario: A click identifies layer and item
- **WHEN** the user clicks on a surface
- **THEN** the layer and item the engine attributes to that hit become the selection

#### Scenario: Clicking empty space clears the selection
- **WHEN** the user clicks where the ray hits nothing
- **THEN** the selection is cleared rather than left on the previous target

### Requirement: Layer visibility and transform are directly editable
The user SHALL be able to toggle a layer's visibility and set its transform, applied through the engine's layer operations. A hidden layer SHALL contribute nothing to the displayed surface and SHALL NOT be pickable.

#### Scenario: Hiding removes contribution
- **WHEN** the user hides a layer that contributes to the surface
- **THEN** the viewport shows the surface without that layer's contribution

#### Scenario: A transform is undoable as one step
- **WHEN** the user sets a layer transform and undoes it
- **THEN** the transform reverts in a single undo step

### Requirement: A layer's cost is inspectable
The application SHALL let the user inspect what a layer's field costs, using the engine's field report, and SHALL present the engine's consolidation estimate before offering to consolidate. It SHALL NOT consolidate without the user asking.

#### Scenario: Cost is shown before consolidation
- **WHEN** the user opens consolidation for a layer
- **THEN** the engine's estimated cost is shown and no consolidation runs until the user confirms

#### Scenario: Consolidation is undoable
- **WHEN** the user consolidates a layer and undoes it
- **THEN** the layer returns to its unconsolidated edit list

### Requirement: Geometry statistics are displayed for the current document
The application SHALL display the current polygon, vertex and triangle counts and the object count for the document, updating after edits that change them.

#### Scenario: Counts follow edits
- **WHEN** an edit changes the meshed geometry
- **THEN** the displayed counts update to the new values

#### Scenario: Counts describe what is displayed
- **WHEN** counts are shown alongside a viewport displaying a reduced level of detail
- **THEN** the counts state which resolution they describe, so a reduced LOD is not read as a smaller model

### Requirement: Mesh layers are carried but not sculpted
The application SHALL allow an imported mesh to be carried by the document as a mesh layer, saved and reloaded with it, and exported alongside sculpted content. Sculpting tools SHALL be disabled on mesh layers, with the reason stated.

#### Scenario: A mesh layer round-trips
- **WHEN** a document containing an imported mesh layer is saved and reopened
- **THEN** the mesh layer is present with its geometry unchanged

#### Scenario: Sculpting a mesh layer is refused
- **WHEN** a mesh layer is active and the user selects a sculpting tool
- **THEN** the tool is disabled and states that mesh layers are carried rather than sculpted
