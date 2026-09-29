## ADDED Requirements

### Requirement: An undo's region does not grow with the edits before it
The region an undo re-meshes SHALL be the undone edit's own reach and SHALL NOT grow with the number of edits that came before it on the same layer. A regression test SHALL re-run a twenty-edit series and report each undo's cost split into the bricks it re-meshed and the price of one brick, so that a change to either factor can be told apart from a change to the other.

#### Scenario: Twenty grabs on one patch
- **WHEN** a sculptor makes twenty mirrored Move gestures on one patch of a field layer and undoes the newest after each
- **THEN** every undo re-meshes the grab's neighbourhood rather than the node's bound, and no more than 1.5 times the bricks the first undo re-meshed

#### Scenario: The cost is reported by factor
- **WHEN** the series is measured
- **THEN** the undo time is reported alongside the bricks re-meshed and the time per brick, so that growth from a longer deformer chain is not mistaken for growth in the region

#### Scenario: The 200x class fails the test
- **WHEN** the last undos of the series cost eight times the first or more
- **THEN** the test fails and names both factors
