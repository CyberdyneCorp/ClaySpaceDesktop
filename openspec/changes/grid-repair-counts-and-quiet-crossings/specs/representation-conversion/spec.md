## ADDED Requirements

### Requirement: A crossing does not re-mesh what it left unchanged
A crossing SHALL re-mesh the brick surface as a whole only when it changes the
distance field: when it produces a field layer, or replaces a field layer in
place. Any other crossing SHALL leave the brick surface to the incremental
sync. Drawing the document after a crossing SHALL NOT re-mesh the source grid's
chunks or smooth surface, since the crossing read the grid and changed no cell.

#### Scenario: A grid crossed to a field keeps its mesh
- **WHEN** a sculpted grid is crossed to a field beside it and the document is
  drawn again
- **THEN** no chunk and no smooth surface of the grid is meshed again

#### Scenario: A field crossed to a grid dirties no brick
- **WHEN** a field layer is crossed to a grid beside it
- **THEN** no brick is marked dirty and no whole-surface settle follows
