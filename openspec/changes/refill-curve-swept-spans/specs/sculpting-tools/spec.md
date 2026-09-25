## REMOVED Requirements

### Requirement: Laying a control point costs the end it added
The bounded append contract is removed because it leaves stale cached geometry.

### Requirement: Dragging a control point costs what the drag disturbed
The bounded drag contract is removed because the field can change beyond the
selected point neighbourhood.

## ADDED Requirements

### Requirement: Curve edits preserve the cached surface
Appending, dragging, removing, or changing a curve SHALL mark every brick in
the curve's old and new extents that may contain its field. A zero-displacement
drag SHALL leave the curve and cached surface unchanged.

The implementation SHALL account for changes beyond adjacent control spans,
including a stroke chain's blend and a swept profile's dependence on the total
guide length and transported frames. It SHALL refill before reporting an edit
complete.

A test SHALL compare cached surface geometry with a full refill across
supported joins and radii. A rendered fixture SHALL compare the drawn result
of an incremental drag with one after a full refill.

#### Scenario: A curve is drawn freehand
- **WHEN** control points are appended one after another
- **THEN** each result has the same cached surface as a full refill

#### Scenario: A control point is dragged
- **WHEN** a point is moved and the curve is refilled from scratch afterward
- **THEN** the cached surface geometry and rendered drawing remain unchanged

#### Scenario: A control point stays still
- **WHEN** a selected point receives a zero displacement
- **THEN** the cached surface remains unchanged
