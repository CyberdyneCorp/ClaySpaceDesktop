## MODIFIED Requirements

### Requirement: Layer visibility and transform are directly editable
The user SHALL be able to toggle a layer's visibility and set its transform, applied through the engine's layer operations. A hidden layer SHALL contribute nothing to the displayed surface and SHALL NOT be pickable.

A layer's transform SHALL be settable with the manipulator as well as by any
numeric control the interface offers, and the two SHALL address the same value:
a layer moved by dragging reads back as moved.

Symmetry SHALL follow the layer. The engine reflects a layer's items through
the plane where the local coordinate is zero, and the layer transform carries
that plane with it, so a mirrored layer that is moved stays mirrored about
itself rather than about where it used to be.

#### Scenario: Hiding removes contribution
- **WHEN** the user hides a layer that contributes to the surface
- **THEN** the viewport shows the surface without that layer's contribution

#### Scenario: A transform is undoable as one step
- **WHEN** the user sets a layer transform and undoes it
- **THEN** the transform reverts in a single undo step

#### Scenario: A mirrored layer is moved
- **WHEN** a layer with symmetry on is moved sideways
- **THEN** its two halves stay symmetric about the layer's own plane

## ADDED Requirements

### Requirement: An SDF layer's objects are addressable
An SDF layer's contents SHALL be presentable as a list of the objects it holds,
each of which can be selected, and selecting one in that list SHALL select it in
the viewport and the reverse.

An item that is not an object a sculptor placed — a sculpting stroke, an
armature's skin, a curve's swept form — SHALL be distinguishable from one that
is, so that a list of a worked layer's contents is not a hundred rows of
"stroke".

#### Scenario: Placed objects are listed
- **WHEN** a layer holds two placed objects and forty strokes
- **THEN** the two objects are listed and reachable, and the strokes do not
  each take a row

#### Scenario: Selection agrees between the list and the viewport
- **WHEN** an object is picked in the viewport
- **THEN** the same object is selected in the list, and a manipulator appears
  on it
