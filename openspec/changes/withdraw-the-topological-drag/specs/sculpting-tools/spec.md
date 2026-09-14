## REMOVED Requirements

### Requirement: Mover Topológico is offered on SDF layers
**Reason**: The verb's weight is a geodesic distance solved on the bake's own
lattice, and the lattice's metrication error — 1–3%, grid-locked, one cell in
wavelength — is multiplied by the drag and lands on the surface as relief of
about a third of a cell. On the reported stroke that renders as a square patch
of stair-stepping, 2.07x rougher than the sphere it was drawn on where every
other brush on a field is between 1.01x and 1.31x. No parameter the application
passes changes it: the bake and replace alone are clean to four decimals, and
feather, band and cell were each swept with no improvement.

**Migration**: Mover is the drag that remains on a field. It is Euclidean, so
it reaches across a gap where the withdrawn tool did not; a sculptor who needs
the parts kept apart masks one of them.

The application offered a drag on SDF layers whose falloff was weighted by
distance measured along the material rather than through space, bound to
`clay_item_volume_move_topological`.

#### Scenario: A topological drag does not reach across a gap
- **WHEN** Mover Topológico is dragged on a form whose two parts are close in
  space and far along the surface
- **THEN** only the part under the brush moves, where the Euclidean Mover at the
  same radius moves both

## ADDED Requirements

### Requirement: A brush on a field leaves a surface rather than its lattice
No tool the shelf offers on an SDF layer SHALL leave behind the sampling
lattice of any volume it bakes. A stroke's result SHALL be measured as it is
rendered, against an untouched form rendered the same way in the same run,
because the lattice is a defect of shading and silhouette rather than of
displacement — the withdrawn drag moved the surface by the correct amount while
the surface it left was unusable.

A tool whose rendered result is more than 1.5x rougher than the form it was
drawn on SHALL NOT be offered on that representation.

#### Scenario: Every brush on a field is measured on what it draws
- **WHEN** each tool the shelf offers on an SDF layer is stroked across a clean
  sphere with the same brush and the same gesture
- **THEN** each rendered result is within 1.5x the roughness of the untouched
  sphere, and a tool that is not is named in the failure with its own ratio
