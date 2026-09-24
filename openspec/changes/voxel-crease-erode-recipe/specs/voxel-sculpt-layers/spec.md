## ADDED Requirements

### Requirement: Voxel Crease cuts a narrow groove using the existing erode verb
The application SHALL offer Crease on voxel layers as a pinned recipe over
`clay_voxel_sculpt_inflate` with a spherical, constant-falloff brush, an odd
width between three and seven cells, and a negative amount of two. It SHALL
describe this as erosion rather than edge sharpening.

#### Scenario: Crease crosses a voxel surface
- **WHEN** the user strokes Crease across a filled voxel surface
- **THEN** the surface has a narrow groove and fewer occupied cells

#### Scenario: Grid tool limitations are explained
- **WHEN** the user asks why Smear or Clay is unavailable on a voxel layer
- **THEN** the application explains the lack of colour smearing or gradual
  buildup respectively, and names the available alternatives
