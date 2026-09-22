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

Changing a layer's visibility SHALL re-evaluate the field only where the field
can have changed. The surface cache holds the fold of the visible field layers
and nothing else; a grid's and a carried mesh's visibility is honoured where
the drawn geometry is assembled. So showing or hiding a layer that is not a
field layer SHALL re-evaluate nothing, however much material it holds, and
SHALL still leave it out of what is drawn.

Stepping through the history over a visibility change SHALL re-evaluate the
layers whose visibility that change moved, and SHALL NOT stand in for them with
the active layer. A subtool that a step gives back to the document SHALL be
given back to the drawn surface in the same step.

#### Scenario: Hiding removes contribution
- **WHEN** the user hides a layer that contributes to the surface
- **THEN** the viewport shows the surface without that layer's contribution

#### Scenario: Showing a layer again restores the same surface
- **WHEN** a field subtool is hidden and shown again
- **THEN** the surface over it is the one it had before it was hidden

#### Scenario: A grid's eye costs no field work
- **WHEN** a grid or mesh subtool is hidden or shown
- **THEN** no brick of the field is re-evaluated
- **AND** the subtool leaves, or returns to, the geometry handed to the viewport

#### Scenario: Undoing a solo brings back every subtool it hid
- **WHEN** a subtool is soloed and the sculptor then undoes
- **THEN** every subtool the solo hid is both back in the stack and back on the
  drawn surface, not only the one that was active

#### Scenario: A transform is undoable as one step
- **WHEN** the user sets a layer transform and undoes it
- **THEN** the transform reverts in a single undo step

#### Scenario: A mirrored layer is moved
- **WHEN** a layer with symmetry on is moved sideways
- **THEN** its two halves stay symmetric about the layer's own plane
