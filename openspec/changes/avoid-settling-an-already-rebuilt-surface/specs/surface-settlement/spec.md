## ADDED Requirements

### Requirement: Release settlement reflects geometry provenance
The renderer SHALL retain deferred settlement for geometry combining separate partial meshing requests and SHALL avoid repeating settlement for geometry already produced by a single complete request.

#### Scenario: Mask-only release
- **WHEN** a mask-only stroke ends on a synchronized single-request surface
- **THEN** release performs no field rebuild or geometry upload solely for settlement

#### Scenario: Epoch change already rebuilt the surface
- **WHEN** synchronization replaces the whole stored surface after a live preview becomes the document
- **THEN** a deferred release settle does not rebuild that surface again

#### Scenario: Mixed partial requests
- **WHEN** a partial meshing request is merged into an existing store
- **THEN** deferred settlement remains required
- **AND** exact duplicate compaction does not clear that requirement

#### Scenario: Live gesture guard
- **WHEN** a live gesture is open
- **THEN** owed settlement remains deferred until the gesture permits it

#### Scenario: Empty rebuild
- **WHEN** a complete rebuild produces no geometry
- **THEN** no further settlement is required

#### Scenario: Complete dirty-key replacement
- **WHEN** a dirty-key request replaces every stored triangle, including a store with empty bookkeeping entries
- **THEN** no old request ownership remains to settle
