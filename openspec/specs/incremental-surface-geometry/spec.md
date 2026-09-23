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

### Requirement: Batched compacted surface layout
Compaction after an ordinary SDF edit SHALL upload vertex and index data in at most one mapped queue write each while preserving indexed geometry and incremental slot headroom. Full rebuilds and preview initialization SHALL retain per-brick uploads without transferring vertex gaps.

#### Scenario: Nonempty bricks and empty keys
- GIVEN a surface containing nonempty bricks and empty keys
- WHEN a compacted layout is uploaded
- THEN each live indexed vertex SHALL retain its complete attribute bits
- AND each index tail SHALL contain only degenerate triangles
- AND bounds SHALL exclude unused vertex headroom
- AND empty keys SHALL consume no slots

#### Scenario: Subsequent incremental edits
- GIVEN a compacted batched layout
- WHEN a brick grows within its reserved headroom
- THEN it SHALL keep its slot and permit an isolated incremental patch

#### Scenario: Full rebuild
- GIVEN a complete SDF surface rebuild
- WHEN its GPU buffers are populated
- THEN only live vertex runs SHALL be uploaded
- AND unused vertex headroom SHALL NOT add transfer bytes

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

### Requirement: Surface buffers are reused rather than reallocated
A layout SHALL keep a vertex or index buffer whose capacity already holds what
it reserves, and SHALL allocate only the buffer that has run out of room. A
buffer that is grown SHALL be grown geometrically, and never past what the
device will create.

A reservation that is kept SHALL leave the buffer's capacity at the larger
figure rather than the figure reserved, so a surface that shrinks and grows
again within one stroke allocates nothing.

Every path that reserves SHALL write the whole range it then draws, so that a
kept buffer's previous contents are unreachable from the draw call.

#### Scenario: A layout of an unchanged surface
- **WHEN** a surface is laid out again at the size it already had
- **THEN** no GPU buffer is created

#### Scenario: A layout that outgrows its buffers
- **WHEN** a layout reserves more vertices than the vertex buffer holds, and no
  more indices than the index buffer holds
- **THEN** only the vertex buffer is replaced

#### Scenario: A reservation past the device ceiling
- **WHEN** a layout reserves more than the device will create
- **THEN** the reservation is refused and both buffers are left as they were

### Requirement: A settle writes contiguous ranges rather than keys
The incremental upload path SHALL place every touched key before writing any of
them, and SHALL merge spans whose destinations abut into a single buffer write.
The number of writes a settle takes SHALL be proportional to the number of
contiguous changed ranges rather than to the number of keys touched.

Spans separated by a gap SHALL be written separately, so that a merge never
covers a key the settle was not asked to touch.

A merged write SHALL place exactly the bytes the separate writes would have
placed, at the same destinations.

#### Scenario: Keys placed back to back
- **WHEN** a settle writes three keys whose spans meet exactly
- **THEN** each buffer takes one write, carrying the same bytes the three
  separate writes would have carried, and the surface draws identically

#### Scenario: An untouched key between two touched ones
- **WHEN** a settle writes two keys with an unwritten span between them
- **THEN** the two are written separately

#### Scenario: A full rebuild
- **WHEN** the whole surface is laid out afresh
- **THEN** it keeps its per-brick writes, which were measured to cost less
  whole-action latency than batching them

### Requirement: Upload staging is reclaimed every frame
The renderer SHALL poll the device once per rendered frame, after the frame's
work is submitted and without waiting for it, so that completed submissions are
observed and the staging memory their buffer writes took is released within a
bounded number of frames.

#### Scenario: A session of many settles
- **WHEN** frames are rendered continuously while the surface is edited
- **THEN** the staging memory of a settle's writes is released rather than held
  for the life of the session

### Requirement: The device reports what it allocated and how it was written
The graphics device SHALL count mesh buffer allocations, and buffer writes
beside the bytes those writes carried, each readable and resettable, so that
buffer reuse and write merging can be asserted rather than described.

#### Scenario: Reading the counters
- **WHEN** a caller takes the allocation or write count
- **THEN** it receives what has accumulated since it last took it, and the
  count resets
