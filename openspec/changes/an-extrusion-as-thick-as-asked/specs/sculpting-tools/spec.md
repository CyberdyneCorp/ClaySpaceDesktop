## MODIFIED Requirements

### Requirement: A mask can be extruded into a new solid
The application SHALL expose mask extrude, producing a solid from the masked region, with outward, inward and centred options and a roundable rim, applied through the engine's mask-extrude entry point.

On a field layer the region handed to the engine SHALL be the painted patch swept along the layer's own surface normal across the band the chosen side fills, so that the wall's height is the requested thickness wherever the patch is, rather than being capped by how far the painted mask reaches off the surface. A thickness whose region could not be searched within the application's cell budget SHALL be refused with a reason and SHALL leave no layer behind.

#### Scenario: Extract from a mask
- **WHEN** the user extrudes a painted mask outward with a rim radius
- **THEN** a new solid corresponding to the masked patch is added to the document, with the rim rounded as requested

#### Scenario: The wall is as thick as asked
- **WHEN** a patch of a field layer, frozen by an outline or by a brush, is extruded outward at 0.05, 0.1 or 0.6
- **THEN** the wall measured along the surface normal inside the patch is within 10% of the thickness

#### Scenario: The wall's top is even
- **WHEN** a patch is extruded outward at a thickness taller than the mask reaches off the surface
- **THEN** the wall's height varies by no more than 0.02 across the patch

#### Scenario: A wall past the budget is refused
- **WHEN** an extrusion is asked for at a thickness far larger than the patch
- **THEN** it is refused with a reason naming the thickness, and no layer is added
