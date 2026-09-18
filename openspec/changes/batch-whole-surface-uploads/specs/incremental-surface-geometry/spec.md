## ADDED Requirements

### Requirement: Batched compacted surface layout
Compaction after an ordinary SDF edit SHALL upload vertex and index data in at most one mapped queue write each while preserving indexed geometry and incremental slot headroom. Full rebuilds and preview initialization SHALL retain per-brick uploads without transferring vertex gaps.

#### Scenario: Nonempty bricks and empty keys
- GIVEN a surface containing nonempty bricks and empty keys
- WHEN a compacted layout is uploaded
- THEN each live indexed vertex SHALL retain its complete attribute bits
- AND each index tail SHALL contain only degenerate triangles
- AND bounds SHALL exclude unused vertex headroom
- AND empty keys SHALL consume no slots

#### Scenario: Subsequent incremental edits
- GIVEN a compacted batched layout
- WHEN a brick grows within its reserved headroom
- THEN it SHALL keep its slot and permit an isolated incremental patch

#### Scenario: Full rebuild
- GIVEN a complete SDF surface rebuild
- WHEN its GPU buffers are populated
- THEN only live vertex runs SHALL be uploaded
- AND unused vertex headroom SHALL NOT add transfer bytes
