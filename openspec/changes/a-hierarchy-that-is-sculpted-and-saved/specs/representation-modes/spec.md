## ADDED Requirements

### Requirement: A subdivision hierarchy is a subtool that can be worked on
The application SHALL hold a subdivision hierarchy on a layer, draw it from its
display level, place a pointer on it, and accept the brushes the tool table
offers there. Nothing about working on a hierarchy SHALL require the sculptor
to know that its cage is also a mesh layer in the document.

Detail stored on a hierarchy SHALL survive a change to the form beneath it: an
edit at a coarse level SHALL move the frames finer detail is stored in, so that
detail arrives at its new place at its own size rather than being smeared,
flattened or left pointing where the world is.

#### Scenario: Detail rides the form it stands on
- **WHEN** the user sculpts detail at a fine level and then edits the form at a
  coarse one
- **THEN** the detail is still there, the same size, on the same part of the
  surface, oriented to the form as it now sits

#### Scenario: The pointer lands on the surface being drawn
- **WHEN** the user points at a hierarchy that has been sculpted
- **THEN** the brush is placed on the level the viewport is drawing, and not on
  the cage beneath it

#### Scenario: A dab is seen
- **WHEN** a dab lands on a hierarchy
- **THEN** the viewport draws the changed surface, including after the
  hierarchy's rebuildable caches have been released underneath it

### Requirement: A mesh becomes a cage only if it can be one
Crossing a mesh into a hierarchy SHALL refuse a mesh that cannot stand as a
subdivision cage, and SHALL name the fault rather than reporting a failure.
Nothing SHALL be repaired: mending a cage silently changes retopology the
sculptor paid for.

A refused crossing SHALL leave the source layer exactly as it was.

#### Scenario: A mesh with a degenerate face is refused by name
- **WHEN** the user crosses a mesh carrying a face with repeated or collinear
  corners into a hierarchy
- **THEN** the crossing is refused, and the sentence names that fault, so the
  sculptor goes back to the mesh rather than looking for a setting

#### Scenario: The source survives the refusal
- **WHEN** a crossing into a hierarchy is refused
- **THEN** the source layer is still a mesh layer and still carries its
  triangles

### Requirement: Adding a level is priced and refused rather than attempted
The application SHALL state what adding a level would cost before it is added,
and SHALL refuse a level that does not fit. The figure stated SHALL be the
**peak** during the build rather than what remains after it, because on a
constrained machine it is the high-water mark that ends the session.

A refused level SHALL leave the hierarchy exactly as deep as it was.

#### Scenario: The cost is available before the button is pressed
- **WHEN** a hierarchy is the active layer
- **THEN** what one more level would occupy is available without adding one

#### Scenario: A level that does not fit is refused
- **WHEN** the user asks for a level whose peak exceeds the budget
- **THEN** the request is refused with the peak and the budget stated, and the
  hierarchy holds the levels it held before
