## ADDED Requirements

### Requirement: Lossless compact triangle identifiers
Exact triangle pruning SHALL preserve the same surviving owner, index order, winding and vertex bits when using packed triangle identifiers and in-place compaction.

#### Scenario: IDs within the packing bound
- GIVEN the total input vertex count is at most u32::MAX
- WHEN sorted interned vertex IDs are packed into triangle keys
- THEN every distinct sorted ID triple SHALL have a distinct packed representation
- AND collisions in hashing SHALL still use full key equality

#### Scenario: Wide IDs
- GIVEN the input vertex count exceeds the packing bound
- WHEN exact triangles are pruned
- THEN the original machine-sized ID representation SHALL be used without truncation

#### Scenario: Stable compaction
- GIVEN the input contains at least one complete triangle, duplicate triangles and an incomplete trailing index group
- WHEN indices are compacted in place
- THEN retained complete triangles SHALL have the same order and winding as the original reference
- AND the incomplete trailing group SHALL be discarded as before
