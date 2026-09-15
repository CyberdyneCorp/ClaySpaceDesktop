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
- **AND** release MAY satisfy that requirement by exact duplicate compaction when all stored triangles use current document-gradient shading at full resolution
- **AND** otherwise release retains the full rebuild path

#### Scenario: Live gesture guard
- **WHEN** a live gesture is open
- **THEN** owed settlement remains deferred until the gesture permits it

#### Scenario: Empty rebuild
- **WHEN** a complete rebuild produces no geometry
- **THEN** no further settlement is required

#### Scenario: Complete dirty-key replacement
- **WHEN** a dirty-key request replaces every stored triangle, including a store with empty bookkeeping entries
- **THEN** no old request ownership remains to settle

#### Scenario: Exact release compaction
- **WHEN** a synchronized full-resolution document surface contains only document-gradient geometry and no cage preview
- **THEN** release removes only duplicate triangles with identical complete vertex attributes without invoking engine meshing
- **AND** later incremental edits retain the same distinct triangle set as a full rebuild
- **AND** settlement telemetry identifies compaction and reports zero engine mesh and mesh-read time

#### Scenario: Explicit rebuild
- **WHEN** an explicit settle or rebuild is requested
- **THEN** the renderer retains its full rebuild semantics

#### Scenario: Reclaim removed geometry
- **WHEN** release compaction removes duplicate triangles or retains emptied brick entries
- **THEN** empty entries and vertices referenced by no surviving triangle are discarded
- **AND** surviving triangle order and every referenced vertex attribute remain unchanged
