## ADDED Requirements

### Requirement: A smooth on a hierarchy is made at the frequency chosen for it
A smooth stroke on a hierarchy SHALL be made through the engine's layered
stroke entry point carrying a stated frequency, and SHALL NOT be made as a
stamp carrying the smoothing brush, which is the plain Laplacian over positions
and removes detail it passes over.

The frequency SHALL be one of three: the form, the detail alone, or the form
with the detail re-applied unchanged. It SHALL default to the form with the
detail, which is what the tool's own caveat describes and the one of the three
that no other representation can offer.

The choice SHALL be offered on a hierarchy and on no other representation,
since the other three store one surface and therefore have one smooth, and it
SHALL be offered only while a smoothing tool is in hand.

Where a pass is active the smooth SHALL be written to that pass; where none is,
it SHALL be written to the form under them — the same rule every other stroke
on a hierarchy follows.

#### Scenario: A smooth that keeps the detail it passes over
- **WHEN** a sculptor smooths a region of a hierarchy carrying detail, at the
  form-with-detail frequency
- **THEN** the detail is left standing at its own height
- **AND** the form beneath it has moved

#### Scenario: A smooth that takes the detail off
- **WHEN** the same region is smoothed at the form frequency
- **THEN** the detail is lowered, as a plain Laplacian over it lowers it

#### Scenario: A representation with one smooth
- **WHEN** the smoothing tool is in hand on a field, a grid or a mesh layer
- **THEN** no frequency is offered

#### Scenario: Another tool on a hierarchy
- **WHEN** a tool that does not smooth is in hand on a hierarchy
- **THEN** no frequency is offered

#### Scenario: What a sculptor who has not chosen gets
- **WHEN** a smooth is made on a hierarchy and no frequency has been chosen in
  this session
- **THEN** it is made at the form with the detail carried through unchanged
