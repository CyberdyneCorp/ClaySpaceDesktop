## ADDED Requirements

### Requirement: A brush size means the same reach on every representation
A brush of a given size SHALL act over the same radius on a voxel layer as on a
field layer.

The engine's voxel footprint is a span across, and a brush size is a radius, so
the host SHALL convert between them rather than passing one as the other.

#### Scenario: The same drag on a grid and on a field
- **WHEN** the same Move drag is made at the same brush size on a voxel layer and
  on a field layer
- **THEN** the surface on each moves the same distance, to within one cell of the
  grid

#### Scenario: Reach scales with the brush
- **WHEN** a drag on a voxel layer is made far past the brush radius at two brush
  sizes
- **THEN** the distance it saturates at doubles when the brush size doubles

#### Scenario: A brush past the size ceiling
- **WHEN** a voxel brush is set larger than 32 cells of radius
- **THEN** the dab stops growing at 64 cells across, and the documentation says so
