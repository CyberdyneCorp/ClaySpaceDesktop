## ADDED Requirements

### Requirement: Whole SDF layer dragging keeps its first frame responsive
When a whole SDF subtool is dragged, the application SHALL defer field evaluation until the drag ends. If exactly one layer is visible, the viewport SHALL transform its retained surface to follow the manipulator during the gesture. Repeated preview updates SHALL derive from the original surface. On release, the preview SHALL clear and the viewport SHALL show the field evaluated at the final transform.

#### Scenario: First drag of a single visible SDF layer
- **WHEN** the sculptor starts moving a whole SDF layer and drags its manipulator
- **THEN** the visible surface follows the manipulator without a field refill or a surface mesh upload on that drag frame
- **AND** the released surface reflects the final document transform

#### Scenario: Other layers are visible
- **WHEN** the sculptor drags a whole SDF layer among other visible layers
- **THEN** the manipulator follows the pointer while the composite surface waits for the final evaluation, leaving the other layers visually fixed
