## ADDED Requirements

### Requirement: Mask strokes refresh only their affected surface
A mask stroke SHALL report its changed world-space region and the viewport SHALL sample mask attributes only on intersecting surface keys. Whole-mask operations, history steps, and layer changes SHALL refresh the full surface. A refused mask operation SHALL NOT schedule a refresh.

#### Scenario: A local mask stroke
- **WHEN** a brush paints a local mask region
- **THEN** keys outside the stroke's influence box are not sampled or uploaded
- **AND** the final mask attributes match a full refresh bit for bit

#### Scenario: A mask operation is refused
- **WHEN** an invert is requested with no active mask
- **THEN** the mask revision does not change
