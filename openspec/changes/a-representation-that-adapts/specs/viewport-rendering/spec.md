## ADDED Requirements

### Requirement: Dynamic uploads follow independent revisions
The viewport SHALL use DynamicSurface dirty chunks and separate topology,
geometry and attribute revisions to update only affected GPU data. A changed
topology SHALL replace its affected chunk layout, a geometry-only change SHALL
update positions and normals, and an attribute-only change SHALL update
attributes. Deleted chunks SHALL no longer draw. Camera movement alone SHALL
not upload surface data.

#### Scenario: A local edit uploads locally
- **WHEN** a Dynamic stroke changes one chunk
- **THEN** unchanged chunks retain their GPU data and the incremental frame matches a full rebuild

#### Scenario: Undo invalidates changed chunks
- **WHEN** a topology-changing edit is undone
- **THEN** the chunks whose connectivity changed are refreshed and the drawing matches the restored surface
