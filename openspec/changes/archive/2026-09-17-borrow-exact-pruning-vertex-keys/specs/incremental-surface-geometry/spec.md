## ADDED Requirements

### Requirement: Borrowed exact vertex identity during pruning
Triangle pruning SHALL compare all ten vertex float bit representations while borrowing immutable vertex storage for temporary interning.

#### Scenario: Equal values in separate allocations
- GIVEN matching vertex bits stored in different bricks
- WHEN duplicate triangles are pruned
- THEN the same sorted first owner SHALL survive regardless of storage address or hash collisions

#### Scenario: Special float bits
- GIVEN vertices differing only by signed zero or NaN payload in any channel
- WHEN exact identities are interned
- THEN these vertices SHALL remain distinct exactly as in the full-bit reference

#### Scenario: Release storage changes after pruning
- GIVEN a release removes unused vertices and empty bricks
- WHEN pruning is repeated after storage compaction
- THEN surviving geometry SHALL remain identical to the independent reference
- AND borrowed interning keys SHALL NOT outlive their pruning pass
