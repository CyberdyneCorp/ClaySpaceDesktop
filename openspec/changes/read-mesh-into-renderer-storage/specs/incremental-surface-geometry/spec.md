## ADDED Requirements

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
