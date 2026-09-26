## ADDED Requirements

### Requirement: Mesh deformers honor the active painted mask
Mesh taper and twist SHALL leave fully masked vertices in place. Unmasked vertices SHALL receive the deformation. The lattice cage remains a whole form control point operation.

#### Scenario: Taper a masked mesh
- **WHEN** a mesh has a fully painted mask and taper is applied
- **THEN** its masked vertices SHALL keep their positions
