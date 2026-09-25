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
