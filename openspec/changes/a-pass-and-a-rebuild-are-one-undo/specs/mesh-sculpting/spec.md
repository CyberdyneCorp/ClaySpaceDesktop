## ADDED Requirements

### Requirement: A rebuild is refused while a gesture is open
A rebuild replaces every vertex and every index of the layer it is asked about.
An open gesture holds an adjacency, a spatial index and an exact record of what
it has moved over those same triangles, so the application SHALL refuse a
rebuild while a gesture is open, SHALL say so in a sentence that names the
stroke, and SHALL leave the layer byte-identical.

#### Scenario: A rebuild mid-stroke is refused
- **WHEN** a gesture is open on a mesh layer and a rebuild is asked for
- **THEN** it is refused with a reason, and the layer is exactly as the gesture
  left it

#### Scenario: The same rebuild goes through once the stroke is finished
- **WHEN** the gesture ends and the rebuild is asked for again
- **THEN** it happens
