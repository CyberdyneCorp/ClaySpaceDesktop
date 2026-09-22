## ADDED Requirements

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
