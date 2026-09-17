## ADDED Requirements

### Requirement: Batched fresh surface layout
Fresh whole-surface layouts SHALL upload staged vertex and index arrays in at most one queue write each while preserving the indexed geometry and incremental slot headroom.

#### Scenario: Nonempty bricks and empty keys
- GIVEN a surface containing nonempty bricks and empty keys
- WHEN a fresh layout is uploaded
- THEN each live indexed vertex SHALL retain its complete attribute bits
- AND each index tail SHALL contain only degenerate triangles
- AND bounds SHALL exclude unused vertex headroom
- AND empty keys SHALL consume no slots

#### Scenario: Subsequent incremental edits
- GIVEN a fresh batched layout
- WHEN a brick grows within its reserved headroom
- THEN it SHALL keep its slot and permit an isolated incremental patch
