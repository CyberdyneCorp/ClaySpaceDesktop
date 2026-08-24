## MODIFIED Requirements

### Requirement: A tool unavailable on the active layer is disabled with a reason
The application SHALL offer, for the active layer, the tools that exist for that
layer's representation, and SHALL NOT offer a tool that the representation has
no verb for. Where a tool exists for the representation but cannot be used on
this layer right now — the layer is protected or hidden, or a prerequisite such
as a colour attribute is missing — the application SHALL show it disabled and
state which of those it is.

This replaces the previous rule that every tool is shown for every layer and
disabled with a reason where it does not apply. With three representations
carrying substantially different vocabularies, a single list would be mostly
disabled entries whatever the active layer, and the reason would be the same
sentence on all of them: absence carries that better than a greyed row does.

#### Scenario: A tool with no verb here is absent
- **WHEN** the active layer's representation has no verb for a tool
- **THEN** the tool is not offered for that layer

#### Scenario: A protected layer disables its own tools
- **WHEN** the active layer is protected
- **THEN** the tools that exist for its representation are shown disabled and
  name the protection

#### Scenario: A missing prerequisite is named
- **WHEN** a tool requires an attribute the active layer does not carry
- **THEN** the tool is shown disabled and names the missing attribute

## ADDED Requirements

### Requirement: The engine's combine operations and blend profiles are selectable
The application SHALL let a sculptor choose the combine operation an SDF edit
uses from those the engine provides, and the blend profile it is applied under.

#### Scenario: An operation is chosen
- **WHEN** the user chooses a combine operation before making an edit
- **THEN** the edit is recorded with that operation

#### Scenario: A blend profile is chosen
- **WHEN** the user chooses a blend profile
- **THEN** edits made under it use that profile

### Requirement: Alphas modulate a stamp
The application SHALL let a sculptor supply a scalar stamp pattern and apply it
through a brush on the representations where the engine accepts one. The
application SHALL state where an alpha is not available.

#### Scenario: An alpha is stamped
- **WHEN** the user applies a brush carrying an alpha
- **THEN** the surface shows the pattern under the brush's falloff

#### Scenario: An alpha where none is accepted
- **WHEN** the active representation accepts no alpha
- **THEN** the alpha control is unavailable and says so

### Requirement: Deformers act on a layer as authoring operations
The application SHALL offer the engine's deformers as operations on a layer,
distinct from brushes, with the parameters each takes and without requiring a
brush position.

#### Scenario: A deformer is applied
- **WHEN** the user applies a deformer to a layer with its parameters
- **THEN** the layer's form changes accordingly and the operation is one undo
  step

### Requirement: A voxel grid can be repaired before baking
The application SHALL offer the engine's pre-bake repair on a voxel layer:
reporting what is wrong, closing holes, and filling voids. The report SHALL be
shown before any repair is applied.

#### Scenario: A report precedes a repair
- **WHEN** the user opens repair on a voxel layer
- **THEN** the count of holes and voids is stated before anything is changed

#### Scenario: Holes are closed
- **WHEN** the user closes holes on a pierced shell
- **THEN** the report afterwards states fewer holes

### Requirement: Masks gate operations, not only brushes
The application SHALL apply a painted mask to any operation the engine can gate,
including combine operations, and not only to brush strokes.

#### Scenario: A mask protects against a boolean
- **WHEN** a region is masked and a subtracting edit crosses it
- **THEN** the masked region is not cut

### Requirement: Held keys substitute the verb and the sign for one gesture
The application SHALL let a sculptor smooth or take material away with the tool
already in hand, by holding a key for the length of one stroke, without
changing what the shelf has selected.

The keys SHALL be read at the press and held for the whole gesture, so a key
caught or released mid-drag does not change the verb under the sculptor's hand.

Inverting SHALL mean what it means on the active representation: a field turns
its combine operation over, a mesh negates its brush strength, and a grid
erases rather than deposits. An operation with no opposite SHALL be left as it
is rather than becoming a different verb.

#### Scenario: Shift smooths whatever is selected
- **WHEN** a stroke is begun with Shift held while a build-up tool is selected
- **THEN** every segment of that stroke smooths
- **AND** the next stroke, made without the key, builds up again

#### Scenario: The invert key digs on every representation
- **WHEN** the same stroke is made with the invert key held
- **THEN** a field is cut where it would have been raised
- **AND** a mesh vertex moves inward where it would have moved outward
- **AND** a grid's cells are cleared where they would have been set

### Requirement: A mask is painted and seen on every representation
The application SHALL offer the mask tool on SDF, voxel and mesh layers alike,
and painting one SHALL freeze a region rather than change the surface.

The frozen region SHALL be drawn over the surface it protects, on both the
brick-cache surface and the carried mesh and voxel layers.

A single key SHALL start mask painting and put the previous tool back.

#### Scenario: The mask tool freezes rather than deposits
- **WHEN** the mask tool is stroked across a voxel layer
- **THEN** a mask exists afterwards
- **AND** no material was added to the grid

#### Scenario: A painted mask is visible
- **WHEN** a mask is painted on the surface
- **THEN** the drawn surface is darker where the mask covers it
- **AND** clearing the mask returns the surface to what it was

#### Scenario: An edit beside a mask does not erase what is drawn
- **WHEN** a stroke re-meshes bricks that carried mask shading
- **THEN** the frozen region is still drawn afterwards
