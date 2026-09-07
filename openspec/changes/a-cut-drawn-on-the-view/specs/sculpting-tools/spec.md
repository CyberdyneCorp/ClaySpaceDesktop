## ADDED Requirements

### Requirement: Trim cuts with a shape drawn on the view
Trim SHALL resolve a shape drawn over the model into an edit item that cuts
through the form, and SHALL NOT be driven as a stroke across the surface.

The cut SHALL be a prism rather than a frustum. A shape drawn under a
perspective camera sweeps a converging wedge, and cutting with one gives a cut
face that is not flat and a solid that depends on where the camera stood.

The shape SHALL reach the engine as a frame — an origin and an orthonormal
basis — together with the outline in **world units** on that frame. The engine
holds no viewport, so the crossing from what a sculptor drew to what the engine
cuts is this application's to make and to get right.

The sweep SHALL be derived from the region being cut rather than given
explicitly, so that a cut passes all the way through the form instead of
stopping inside it.

#### Scenario: A shape is drawn across a form
- **WHEN** a Trim gesture is completed over a field subtool
- **THEN** the form is cut by a straight prism whose cross-section is the shape
  that was drawn, and the cut face is flat

#### Scenario: The same shape is drawn from the same angle twice
- **WHEN** two identical gestures are made from one camera position
- **THEN** they produce the same cut, independent of what lay under the pointer
  when each began

### Requirement: An open stroke and a closed lasso are different cuts
An open stroke drawn across the form SHALL be closed against the frame's own
bounds on the side it covers. A closed lasso SHALL be tessellated as the
outline it is.

These SHALL be separate entry points chosen by the gesture, not one path with a
flag. Joining an open stroke's endpoints cuts a sliver between them rather than
dividing the frame — the same points, a different shape — so a toggle would
silently change what a drawn line means.

A rectangle gesture SHALL use the engine's own rectangle rather than a
four-point polygon, since the shape can be expressed exactly.

#### Scenario: A line is drawn across a form
- **WHEN** an open Trim stroke crosses the form
- **THEN** the material on one side of it is removed, rather than a sliver
  between the stroke's two ends

### Requirement: Which half survives is the operation
The half of the frame an open stroke's outline covers SHALL be inferred from
the direction the stroke was drawn. Its **fate** SHALL be the operation:
subtract removes what the shape covers, intersect keeps only it.

The inversion modifier SHALL flip the operation and SHALL NOT flip the side.
Two controls that both mean "the other half" are two ways to say one thing, and
the engine's own note rejects that for the same reason.

A closed lasso has no side to infer and SHALL take only the operation.

#### Scenario: A sculptor inverts a trim
- **WHEN** the inversion modifier is held during a Trim gesture
- **THEN** the surviving half is the other one, and the shape covered is
  unchanged

### Requirement: Trim is offered only where it can act
Trim SHALL be offered on a field subtool and refused on a mesh, a grid and a
hierarchy, because the cut resolves to a field item.

It SHALL grey out rather than acting through a conversion. Crossing a mesh into
a field and back is a representation change with its own cost and its own undo
entry, and a tool that says it removes material must not perform one silently.

#### Scenario: A mesh subtool is active
- **WHEN** the shelf is read with a mesh subtool active
- **THEN** Trim is shown unavailable, and the reason names the representation

### Requirement: A cut is an item, not a bake
A completed Trim SHALL place an ordinary item in the active layer and SHALL
record one undo entry.

It SHALL NOT resolve the cut into the layer. A resolved operation is a choice a
sculptor makes, not one a tool makes for them — the same position this
application takes on booleans — and an item can be adjusted afterwards where a
bake cannot.

#### Scenario: A trim is undone
- **WHEN** a completed Trim is undone
- **THEN** one entry is spent and the form returns to what it was
