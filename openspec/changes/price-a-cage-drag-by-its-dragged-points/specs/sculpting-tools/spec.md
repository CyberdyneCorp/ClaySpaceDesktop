## MODIFIED Requirements

### Requirement: A mesh cage shows the bend while it is dragged
The application SHALL show what a lattice cage would do to a mesh layer while
its control points are being dragged, without committing to it.

The preview SHALL NOT compound across frames, SHALL leave the gesture one undo
step, and SHALL be taken back when the cage is abandoned.

The preview SHALL be the engine's own bend of the mesh, so that applying the
cage lands exactly what was last shown — every vertex position and normal, bit
for bit. An optimisation of the preview SHALL NOT approximate it.

A drag frame SHALL be priced by the control points that stand away from rest
rather than by the number of points the cage holds: a single point dragged on
the largest cage SHALL hold a 16 ms frame on the reference mesh, and SHALL cost
within a small multiple of the same drag on the smallest cage. The last frame's
cost and the number of dragged points SHALL be reported with the cage.

#### Scenario: The form follows the cage
- **WHEN** a control point is dragged
- **THEN** the drawn surface has moved before anything is applied
- **AND** nothing has been recorded in the history

#### Scenario: A long drag lands where a short one does
- **WHEN** a drag arrives over twenty frames rather than one
- **THEN** the form ends in the same place

#### Scenario: Abandoning a cage abandons its preview
- **WHEN** a cage is dragged and then cancelled
- **THEN** the form is exactly as it was

#### Scenario: What was shown is what lands
- **WHEN** a cage is dragged over several frames and then applied
- **THEN** every vertex position and normal is bit-identical to the last preview frame

#### Scenario: One corner of the largest cage
- **WHEN** one control point of a 32×32×32 cage is dragged on the reference mesh
- **THEN** each drag frame holds a 16 ms budget
- **AND** the cage reports one dragged point and what the frame cost

## ADDED Requirements

### Requirement: A cage is sized from where the form stands now
A cage SHALL be sized from the active layer's bounds at the moment it is put
up, after any move of the layer or of the forms in it, including whatever
copies the layer's mirror makes.

#### Scenario: A moved form
- **WHEN** a form is moved and a cage is then put around its layer, with the mirror off
- **THEN** the cage is the moved form's box, padded, and not the box it had before the move

#### Scenario: A mirrored form
- **WHEN** a form on a subtool mirrored on X is moved off the axis and a cage is put up
- **THEN** the cage encloses the form and its mirrored copy
