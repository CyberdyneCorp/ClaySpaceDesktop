## ADDED Requirements

### Requirement: Settlement uploads only the geometry it changed
Release compaction SHALL write to the GPU only the brick keys whose stored
geometry it changed, and SHALL fall back to a full layout only when the
existing layout cannot take the patch. A mask refresh SHALL write only the keys
whose mask weights changed.

#### Scenario: Single-dab release
- **WHEN** a release compacts the duplicates left by a dab's partial requests
- **THEN** the bytes uploaded are those of the keys compaction changed, not the layer
- **AND** the drawn triangle set equals a full rebuild's

#### Scenario: Nothing left to compact
- **WHEN** a release finds no duplicate triangles and no reclaimable vertices
- **THEN** nothing is uploaded

#### Scenario: Clearing an absent mask
- **WHEN** the mask is refreshed while no mask exists and no drawn vertex carries a mask weight
- **THEN** nothing is uploaded

### Requirement: Settlement telemetry accounts for its stages
Every settle route SHALL report engine mesh, mesh read, split, duplicate prune
and upload time as measured for that settle, and the measured stages SHALL NOT
exceed the reported total. A stage a route does not perform SHALL be reported
as zero rather than carried over from an earlier settle.

#### Scenario: Rebuild settle
- **WHEN** a settle rebuilds the surface from its bricks
- **THEN** its upload, split and prune times are non-zero and sum with engine and read time to within the total

#### Scenario: Empty field after a rebuild
- **WHEN** a settle finds an empty field after an earlier settle meshed a surface
- **THEN** it reports zero engine mesh and read time
