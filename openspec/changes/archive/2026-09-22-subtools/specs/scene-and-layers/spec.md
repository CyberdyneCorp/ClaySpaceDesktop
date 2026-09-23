## MODIFIED Requirements

### Requirement: Selection is driven by engine picking
Clicking in the viewport SHALL select through the engine's attributed raycast, resolving to the layer and item under the pointer, honoring ghost and lock states. Selection SHALL be reflected consistently in the viewport, the scene tree and the layer stack.

Selecting a layer — by clicking its geometry in the viewport or its row in the
stack — SHALL make it the active sculpt target: subsequent brush strokes land
on that layer, and tool availability follows its representation. The two ways
of selecting SHALL agree; there is one active layer, not a picked one and a
sculpted one.

#### Scenario: A click identifies layer and item
- **WHEN** the user clicks on a surface
- **THEN** the layer and item the engine attributes to that hit become the selection

#### Scenario: Clicking empty space clears the selection
- **WHEN** the user clicks where the ray hits nothing
- **THEN** the selection is cleared rather than left on the previous target

#### Scenario: Clicking a subtool makes it the sculpt target
- **WHEN** two layers each hold geometry, the first is active, and the user
  clicks the second layer's geometry and then sculpts on it
- **THEN** the dab lands on the second layer and the first is unchanged

#### Scenario: A ghosted subtool does not take the activation
- **WHEN** the user clicks where a ghosted layer's geometry stands in front of
  an ordinary layer's
- **THEN** the ordinary layer behind it becomes active, as the pick already
  passes through ghosts

## ADDED Requirements

### Requirement: A new layer declares its representation
Creating a layer SHALL offer the three representations — SDF, voxel and mesh
where a mesh source is at hand — and the resulting layer SHALL carry the
chosen representation's vocabulary from its first edit. The choice SHALL be
stated at creation rather than requiring a conversion afterwards.

#### Scenario: A voxel subtool is created directly
- **WHEN** the user adds a layer and chooses voxel
- **THEN** the new layer is voxel-backed and the voxel tools are available on
  it without a conversion step

#### Scenario: The default stays what it was
- **WHEN** the user adds a layer without engaging the choice
- **THEN** an SDF layer is created, as before

### Requirement: A subtool can be shown alone
The application SHALL offer a solo gesture on a layer: one action shows only
that layer, and releasing the solo restores the visibility each layer had
before it. Solo SHALL be a viewing convenience — it SHALL NOT change which
layer is active or add entries to the undo history.

#### Scenario: Solo isolates and restores
- **WHEN** three layers are visible, one is hidden, and the user solos a layer
  then releases the solo
- **THEN** during the solo only that layer is shown, and afterwards the three
  are visible and the fourth hidden, exactly as before

#### Scenario: Solo leaves history alone
- **WHEN** the user solos a layer, releases it, and undoes once
- **THEN** the undo applies to the last edit before the solo, not to
  visibility
