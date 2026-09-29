## ADDED Requirements

### Requirement: Release compaction is local to replaced bricks
An ordinary local edit SHALL compact exact duplicate triangles only among the bricks it replaced and their immediate neighbours. The result SHALL match a full rebuild's exact triangles and attributes. An explicit full rebuild SHALL continue to prune all stored bricks.

#### Scenario: A local release on a large surface
- **WHEN** a dab replaces a small set of bricks and the surface settles
- **THEN** duplicate pruning examines those bricks and their adjacent bricks
- **AND** distant geometry is neither re-meshed nor uploaded
