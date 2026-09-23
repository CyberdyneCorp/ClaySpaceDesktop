## ADDED Requirements

### Requirement: Exact per-brick vertex remapping
The application SHALL preserve first-encounter local vertex numbering, triangle order and complete vertex bits when splitting an engine mesh into per-brick geometry.

#### Scenario: Shared global vertices
- GIVEN several consecutive bricks reference overlapping global vertex indices
- WHEN the application splits their triangles
- THEN each brick SHALL receive its own first-encounter local indices
- AND transient mappings from previous bricks SHALL NOT affect its output

#### Scenario: Invalid global index
- GIVEN a triangle references an index outside the returned vertex array
- WHEN the application splits that brick
- THEN the existing local-index and default-placeholder behavior SHALL be preserved
- AND valid referenced vertices SHALL retain all their bits

#### Scenario: Empty replacement
- GIVEN a previously populated brick is replaced by an absent or empty mesh range
- WHEN the application updates its per-brick geometry
- THEN that brick's vertex and index arrays SHALL be cleared
