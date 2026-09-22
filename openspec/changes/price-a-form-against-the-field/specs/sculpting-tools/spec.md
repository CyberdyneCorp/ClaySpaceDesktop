## ADDED Requirements

### Requirement: A curve's thickness is priced against the field
The application SHALL price the region a curve's tube would fill against what
this document's brick cache can hold, and SHALL refuse a thickness over that
budget with a sentence naming both the figure and the limit.

The price SHALL be paid before any control point's radius is written, so that a
refused thickness leaves the guide exactly as it was — every point at the
radius it had, and the tube unchanged.

A guide that has been refused a thickness SHALL remain a guide that can be
edited, have points removed, and be taken down.

#### Scenario: A thickness the field cannot hold
- **WHEN** a sculptor sets a curve radius whose tube would fill more of the
  field than the cache can hold
- **THEN** the thickness is refused, the refusal names what it would fill and
  what the document holds, and every control point keeps its own radius

#### Scenario: An ordinary thickness
- **WHEN** a sculptor sets a curve radius the field can carry
- **THEN** it is taken, and the points under the selection are given it

#### Scenario: A refused curve is still a curve
- **WHEN** a thickness has been refused
- **THEN** the guide can still have points removed and can still be taken down
