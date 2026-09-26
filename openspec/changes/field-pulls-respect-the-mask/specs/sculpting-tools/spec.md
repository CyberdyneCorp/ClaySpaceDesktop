## ADDED Requirements

### Requirement: A Snake Hook pull honours the mask and ends where its path ends
A Snake Hook pull on a field SHALL be gated by the active layer's mask over
the whole volume the tendril reaches, not only at the path's samples, so a
fully masked region is unchanged within the tolerance the stamp verbs meet.

The root of a pull SHALL NOT move as the pull is extended. The rounded tip
SHALL end at the last point of the drawn path, within a voxel, rather than a
tip radius past it. The tendril SHALL join the surface through a fillet rather
than a crease.

#### Scenario: A pull crosses a masked band
- **WHEN** a Snake Hook pull is drawn across a masked band on a field
- **THEN** the masked band is unchanged, and the open surface either side rises
  as it does with no mask

#### Scenario: A pull is extended
- **WHEN** a pull is continued segment by segment
- **THEN** the surface around its root stays where the first segment put it

#### Scenario: A pull ends
- **WHEN** a pull is drawn straight out to a point
- **THEN** the tendril's tip stands at that point, within a voxel

#### Scenario: A tendril meets the surface
- **WHEN** a pull is drawn out of the side of a form
- **THEN** the surface turns smoothly across the join, with no crease

### Requirement: A topological drag lands whole without tearing
Move Topológico on a field SHALL apply a drag longer than its falloff as a
series of steps short enough that none folds the surface, each anchored where
the previous one carried the material. It SHALL be held for the whole gesture
and land once when the pointer comes up, and it SHALL be gated by the active
layer's mask.

The result SHALL be a closed, manifold surface with no tear or spur.

#### Scenario: A long drag
- **WHEN** Mover Topológico is dragged twice its reach out of a surface
- **THEN** the pulled lump rises steadily to where it was dragged, and the
  surface stays one closed, manifold piece

#### Scenario: A drag across a mask
- **WHEN** a topological drag crosses a masked band
- **THEN** the masked band is unchanged and the surface stays closed and
  manifold
