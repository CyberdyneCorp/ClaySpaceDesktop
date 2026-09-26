## ADDED Requirements

### Requirement: A cage reports what its drag frames cost
The cage an agent reads SHALL carry how many of its control points stand away
from rest and, where a mesh preview frame has been drawn, what the last one
cost in milliseconds — so that a slow drag names its cage, the work it was
given and the time it took rather than surfacing only as a stall.

A cage whose preview is drawn by the viewport rather than by bending the layer
— a field's — SHALL report no frame cost rather than zero.

#### Scenario: A dragged mesh cage
- **WHEN** one control point of a mesh cage has been dragged
- **THEN** the cage state reports one dragged point and a positive frame cost

#### Scenario: A field cage
- **WHEN** a control point of a field cage has been dragged
- **THEN** the cage state reports the dragged point and no frame cost
