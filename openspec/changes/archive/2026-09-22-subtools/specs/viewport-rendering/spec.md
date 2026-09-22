## ADDED Requirements

### Requirement: The active subtool reads as active in the viewport
The viewport SHALL make the active layer visually distinguishable from the
other visible layers — by tint, dimming or an equivalent consistent cue — and
the cue SHALL agree with the layer stack's indication at all times. The cue is
presentation only: it SHALL NOT alter the geometry the document holds, meshes
or exports.

#### Scenario: Activation moves the cue
- **WHEN** two layers are visible and the user activates the second
- **THEN** the viewport's cue moves from the first layer's geometry to the
  second's, matching the stack

#### Scenario: The cue stays out of the export
- **WHEN** a document is exported while an active-layer cue is showing
- **THEN** the exported geometry and its attributes carry no trace of the cue
