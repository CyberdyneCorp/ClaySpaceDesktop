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

#### Scenario: The drag is interrupted or the final edit is refused
- **WHEN** a selection or mode command interrupts a pending drag, or the release edit fails
- **THEN** the renderer clears the preview before another command draws the scene
- **AND** an interrupted drag commits to its original target before the new target is selected

### Requirement: A placed object's drag in a field draws the object until release
When a placed object whose operation is union (`Add`) in an SDF layer is dragged, the application SHALL defer the field edit until the drag ends. During the drag, the viewport SHALL draw the object's own surface where the manipulator has taken it, with each mirror image the engine emits for the object. The rest of the field SHALL stay drawn as it was. On release, the preview SHALL clear and the field SHALL be written once with the final transform, as one undo step. While the object's layer is hidden, no preview SHALL be drawn. An object with any other operation SHALL keep the live path. Whether an object takes part in its layer's mirror SHALL survive saving and reopening.

#### Scenario: First drag frame of a placed object
- **WHEN** the sculptor starts dragging a placed sphere on the reference scene
- **THEN** no field brick is refilled or re-meshed on that frame, and the press and the frame together take less than 16.7 ms
- **AND** the sphere is drawn where the manipulator has taken it

#### Scenario: Nothing else moves
- **WHEN** a placed object is dragged in a field
- **THEN** no pixel changes outside the region where the object's preview is drawn

#### Scenario: A mirrored object is drawn with its twin
- **WHEN** an object placed under X symmetry is dragged
- **THEN** its reflected image is drawn moving with it, through the subtool's own plane

#### Scenario: The release lands where the live path lands
- **WHEN** the drag ends
- **THEN** the surface is the one a drag evaluated live leaves, bit for bit on a deterministic backend

#### Scenario: A subtracting operand stays live
- **WHEN** a subtracting object is dragged
- **THEN** the cavity follows the drag while the form keeps up, as before

#### Scenario: A hidden layer's object draws nothing
- **WHEN** a union object whose layer is hidden is dragged
- **THEN** nothing is drawn of it until the layer is shown again

#### Scenario: Mirror participation survives a reopen
- **WHEN** an object placed with symmetry off is saved and the document is reopened
- **THEN** its drag draws it alone, without a reflected image
- **AND** an object placed under X symmetry is still drawn with its twin, as the reopened layer's mirror reflects it
