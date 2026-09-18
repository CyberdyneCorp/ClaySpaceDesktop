# incremental-surface-geometry Specification

## Purpose
Getting the engine's meshed surface into renderer storage without a pass that
rebuilds what has not changed: borrowed vertex identity during pruning, and a
readback that writes where the renderer already keeps its vertices.
## Requirements
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

### Requirement: Direct mesh readback into renderer storage
Mesh readback SHALL preserve all returned vertex bits and index order while using the final renderer vertex allocation as the engine copy destination.

#### Scenario: Colored and uncolored meshes
- GIVEN a mesh with positions and normals, with or without colors
- WHEN renderer vertices are read
- THEN copied attributes SHALL match the existing byte-decoding reference exactly
- AND absent colors SHALL remain white and masks SHALL start at zero

#### Scenario: Empty or incompatible mesh
- GIVEN an empty mesh or a nonempty mesh without required normals
- WHEN renderer vertices are read
- THEN the empty mesh SHALL return empty vectors
- AND the incompatible mesh SHALL retain its copy error

