## MODIFIED Requirements

### Requirement: The viewport renders geometry produced by the engine's meshers
The viewport SHALL draw only geometry the engine produced, through the mesh
readback path, and SHALL NOT reconstruct surfaces itself.

Where the geometry carries per-vertex colour — a voxel layer's palette, a mesh
layer's colour attribute — the viewport SHALL modulate the material with it.
The field surface is meshed without colour and its vertices carry the identity
value, so enabling the modulation SHALL leave a field surface unchanged.

#### Scenario: Painted colour is visible
- **WHEN** a voxel layer is painted and the viewport is drawn
- **THEN** the painted region is drawn in that colour

#### Scenario: A field surface is not tinted by the switch
- **WHEN** the same SDF scene is captured before and after colour modulation is
  enabled
- **THEN** the two images are identical
