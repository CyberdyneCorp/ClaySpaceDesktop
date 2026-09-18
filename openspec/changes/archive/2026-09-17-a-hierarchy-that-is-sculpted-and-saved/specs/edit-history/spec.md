## ADDED Requirements

### Requirement: A gesture on a hierarchy is one undo, and it is exact
A stroke on a subdivision hierarchy SHALL be one step in the same history every
other edit goes into, however many segments drew it. Undoing it SHALL restore
the surface exactly rather than approximately, and redoing it SHALL restore
what was taken back.

Both directions SHALL tell the viewport to draw again. A hierarchy rebuilt from
a record is a different surface that may report the same revision as the one it
replaced, so the application SHALL NOT rely on the engine's counters alone to
notice that it moved.

#### Scenario: A drag is one step
- **WHEN** the user drags a brush across a hierarchy and releases
- **THEN** one undo takes the whole gesture back

#### Scenario: The form comes back exactly
- **WHEN** a gesture on a hierarchy is undone
- **THEN** every vertex is where it was before the gesture, and the viewport
  shows it

#### Scenario: A redo is seen
- **WHEN** an undone gesture on a hierarchy is redone
- **THEN** the surface is what it was after the gesture, and the viewport shows
  it rather than continuing to draw what it last uploaded
