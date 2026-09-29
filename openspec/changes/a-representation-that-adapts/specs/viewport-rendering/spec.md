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

#### Scenario: A geometry-only change sends no indices
- **WHEN** a Dynamic edit moves vertices without changing any chunk's triangles
- **THEN** only the moved chunks' vertices are written and no index is uploaded

#### Scenario: Upload volume follows the dirty chunks
- **WHEN** the same stroke is made on two Dynamic surfaces of the same density, one ten times the other's area
- **THEN** the bytes uploaded per dab are within a factor of two of each other and a small fraction of either surface

#### Scenario: Chunk buffers are reused
- **WHEN** a Dynamic stroke moves chunks without growing them
- **THEN** no chunk buffer is reallocated

### Requirement: A Dynamic surface keeps one sculptor for its life
A Dynamic layer SHALL hold its surface together with one sculptor for as long
as the surface lives, so the spatial index is not rebuilt per stroke segment
and the chunks a stroke dirtied remain in the set the viewport drains. A
surface replaced from bytes SHALL get a new sculptor, and the viewport SHALL
lay it out afresh. When the engine reports that the index has degraded, a
rebuild SHALL be queued for between strokes, and the renumbered chunks SHALL be
laid out afresh.

#### Scenario: The dirty set survives the stroke that made it
- **WHEN** two strokes are made through a Dynamic surface's sculptor
- **THEN** the chunks the first dirtied are still reported after the second

#### Scenario: An index rebuild relays the surface
- **WHEN** a Dynamic surface's index is rebuilt
- **THEN** the drawn region is laid out again and draws every face the surface holds

### Requirement: A coloured Dynamic surface is drawn whole
A Dynamic surface that carries vertex colour SHALL be copied whole when it
moves, because the engine's chunk transport carries no attribute. The
limitation SHALL be documented and SHALL NOT drop the colour.

#### Scenario: Paint survives drawing
- **WHEN** a coloured Dynamic surface is painted
- **THEN** the viewport draws the painted colour

### Requirement: Dynamic uploads are reported
The diagnostics report SHALL state, per upload of the carried buffer that sent
Dynamic data, whether it was a chunk patch or a rebuild, how many chunks and how
many bytes it sent, how many uploads were patched, and the largest.

#### Scenario: A patched dab is visible in the report
- **WHEN** a Dynamic dab is drawn by a chunk patch
- **THEN** the report's adaptive-uploads line names a patched upload with its chunks and bytes
