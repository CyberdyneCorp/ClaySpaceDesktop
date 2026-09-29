## ADDED Requirements

### Requirement: Dynamic history restores connectivity
A topology-changing Dynamic edit SHALL participate in the document's
monotonic history sequence. Undo and redo SHALL restore connectivity,
positions and attributes, not only vertex positions. The application SHALL
reuse the document's history ordering and SHALL NOT introduce a second
independently ordered Dynamic stack.

#### Scenario: Undo restores a changed edge graph
- **WHEN** a Dynamic stroke adds or removes vertices or edges and is undone
- **THEN** the connectivity, geometry and attributes equal the pre-stroke state

#### Scenario: Mixed history remains ordered
- **WHEN** a Dynamic stroke follows a conversion and is undone twice
- **THEN** the first undo restores the pre-stroke Dynamic surface and the second restores the pre-conversion representation

#### Scenario: One gesture is one entry
- **WHEN** a Dynamic gesture is drawn in several segments, mirrored, and changes the topology
- **THEN** the history gains exactly one entry, and undoing it restores the exact pre-gesture triangle indices and positions

#### Scenario: Redo is deterministic
- **WHEN** a Dynamic gesture is undone and redone repeatedly
- **THEN** every undo lands on the same pre-gesture digest and every redo on the same post-gesture digest

#### Scenario: A redone crossing keeps the stroke records above it
- **WHEN** a stroke follows a crossing into Dynamic, both are undone, and both are redone
- **THEN** the surface equals the post-stroke state exactly

### Requirement: Dynamic cancel leaves no partial topology
Cancelling a Dynamic stroke SHALL revert only that stroke's record and SHALL
leave the connectivity and positions exactly as the last committed gesture left
them.

#### Scenario: Cancel after committed strokes
- **WHEN** two Dynamic strokes are committed and a third is cancelled while open
- **THEN** the drawn triangles and positions equal those after the second stroke, and two undo steps remain

### Requirement: Dynamic history memory is reported and bounded
Each Dynamic history record SHALL be weighed — a topology delta by its resident
size, a snapshot by its length — against the carried history budget, oldest
first, and the diagnostics report SHALL state the undo steps the adaptive
surfaces hold and their weight.

#### Scenario: A gesture's record is priced
- **WHEN** a Dynamic gesture is committed
- **THEN** the diagnostics report one adaptive undo step with a non-zero weight

### Requirement: Open-boundary surfaces keep an exact snapshot
While the engine's topology delta does not revert a stroke that reached an open
boundary to a valid half-edge structure, a Dynamic surface with boundary edges
SHALL record its gestures as the surface's serialized state, and a closed
surface SHALL record the engine's delta.

#### Scenario: Record kind follows the boundary
- **WHEN** a gesture is drawn on a closed surface and another on an open sheet
- **THEN** the first is recorded as a topology delta, the second as a snapshot, and both undo to a validating surface with the pre-gesture digest
