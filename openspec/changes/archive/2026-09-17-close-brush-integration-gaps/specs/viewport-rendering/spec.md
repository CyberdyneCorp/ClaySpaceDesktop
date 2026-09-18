## MODIFIED Requirements

### Requirement: The viewport renders geometry produced by the engine's meshers
The viewport SHALL display triangles produced by ClayCore's meshers, reached
through the mesh readback path, and SHALL NOT reconstruct a surface of its own.
It SHALL use the surface-nets preview mesher for interactive display and
marching tetrahedra where a watertight, 2-manifold result is required. The
application SHALL NOT reimplement any distance function, combine operator, blend
profile or deformer in a shading language.

This holds for every shader this renderer gains — occlusion, depth reduction,
bilateral upsample, cavity, tone mapping, studio lighting and shadowing are
display code over a depth buffer and a mesh, and none of them evaluates the
field that produced either.

Where the geometry carries per-vertex colour — a voxel layer's palette, a mesh
layer's colour attribute — the viewport SHALL modulate the material with it.
The field surface is meshed without colour and its vertices carry the identity
value, so enabling the modulation SHALL leave a field surface unchanged.

#### Scenario: No field math in shaders
- **WHEN** the WGSL shader sources are inspected
- **THEN** they contain shading, transform and display code only, and no signed-distance primitive, blend or deformer evaluation

#### Scenario: The displayed surface is the document's surface
- **WHEN** a document is displayed in the viewport and separately meshed through the engine at the same resolution
- **THEN** the two surfaces are the same geometry, because the viewport did not compute one of its own

#### Scenario: Painted colour is visible
- **WHEN** a voxel layer is painted and the viewport is drawn
- **THEN** the painted region is drawn in that colour

#### Scenario: A field surface is not tinted by the switch
- **WHEN** the same SDF scene is captured before and after colour modulation is
  enabled
- **THEN** the two images agree to within the rasteriser's own precision, which
  is not the same as being identical: interpolating a colour that is one at
  every vertex is a ratio of two sums, so a silhouette pixel may still round
  the other way
