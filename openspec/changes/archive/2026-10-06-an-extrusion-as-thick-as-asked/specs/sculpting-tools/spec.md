## MODIFIED Requirements

### Requirement: A mask can be extruded into a new solid
The application SHALL expose mask extrude, producing a solid from the masked region, with outward, inward and centred options and a roundable rim, applied through the engine's mask-extrude entry point.

A field layer SHALL hand the engine its own painted mask, and an extrusion whose measurement of the mask would exceed the application's cell budget SHALL be refused with a reason and leave no layer. A voxel layer SHALL extrude through the engine's voxel verb.

#### Scenario: Extract from a mask
- **WHEN** the user extrudes a painted mask outward with a rim radius
- **THEN** a new solid corresponding to the masked patch is added to the document, with the rim rounded as requested

#### Scenario: The wall is as thick as asked
- **WHEN** a patch of a field layer, frozen by an outline or by a brush, is extruded outward at 0.05, 0.1 or 0.6
- **THEN** the wall measured along the surface normal inside the patch is within 10% of the thickness

#### Scenario: The wall is even along the mask boundary
- **WHEN** a patch of a field layer is extruded outward at 0.1 or 0.6
- **THEN** the wall's height at eight spots just inside the mask boundary varies by no more than 0.005, and the wall's top across the patch varies by no more than 0.02

#### Scenario: A grid's wall reaches the thickness at its crown
- **WHEN** a patch of a voxel layer at the 0.02 cell is extruded outward at 0.05, 0.1 or 0.6
- **THEN** the wall at the middle of the patch is within one cell of the thickness, and a wall of up to ten layers is within half a cell of the same height around the patch's boundary

#### Scenario: A wall past the budget is refused
- **WHEN** an extrusion is asked for at a thickness far larger than the patch
- **THEN** it is refused with a reason naming the thickness, and no layer is added
