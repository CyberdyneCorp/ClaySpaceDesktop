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

### Requirement: A grown layer's cost is reported as a trend to forty edits
The undo series SHALL be carried to forty edits and reported at 1, 10, 20 and 40, and a Snake Hook pull begun on the worked patch SHALL be timed at the same checkpoints, so that cost growing with the edit count shows as a slope rather than as one ratio.

#### Scenario: Forty grabs on one patch
- **WHEN** the series reaches forty mirrored Move gestures
- **THEN** every undo still re-meshes the grab's neighbourhood, and the last three undos cost less than sixteen times the first three

#### Scenario: A pull on a grown layer
- **WHEN** a Snake Hook pull is begun on the patch after forty Move gestures
- **THEN** its first segment and its median segment each cost less than sixteen times the same pull after one gesture
