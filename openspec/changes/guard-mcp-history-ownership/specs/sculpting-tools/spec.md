## ADDED Requirements

### Requirement: Layer operations declare their recovery policy
Every layer operation SHALL declare whether it is undoable or requires consent before it can be offered. Taper, twist, lattice drag, close holes, and fill voids SHALL be undoable. Regional refinement SHALL require consent if offered before an undo record exists.

#### Scenario: A new operation has no implicit safety status
- **WHEN** a layer operation is added to the model
- **THEN** the exhaustive recovery classification requires an explicit choice
