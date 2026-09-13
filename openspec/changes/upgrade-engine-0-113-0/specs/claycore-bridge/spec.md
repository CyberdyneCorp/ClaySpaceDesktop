## MODIFIED Requirements

### Requirement: The pinned ABI and the container minor are constants a test holds
The safe wrapper SHALL name the engine ABI it was written against, and the
`.clayspace` container minor this build writes, as constants that a test checks
against the linked engine rather than against themselves.

Neither SHALL be derived from the linked engine. A constant read back from the
thing it is meant to check makes the check assert that a number equals itself,
and the whole value of both is that moving the submodule without moving them
**fails**.

The container minor SHALL carry, beside it, why this build writes that minor —
including that across a multi-release jump the minor may move in steps that no
single release's notes describe, so a reader meets the reasoning rather than
reconstructing it from one set of notes.

#### Scenario: The pin moves and the constants do not
- **WHEN** the vendored engine is pointed at a release whose ABI minor or
  container minor differs from the constants
- **THEN** the wrapper's own tests fail, naming both numbers and saying which
  one moved

#### Scenario: A file says what the build claims
- **WHEN** this build writes a `.clayspace`
- **THEN** the minor in the file's own header is the minor the constant claims

## ADDED Requirements

### Requirement: Every result code the pinned header declares reaches a named kind
The wrapper SHALL name every `clay_result` code the pinned header declares, and
a test SHALL compare that vocabulary against the header on disk rather than
against itself.

A code the wrapper does not name SHALL still be carried verbatim rather than
guessed at, so an unknown result is reported as the number it was.

#### Scenario: The engine gains a result code
- **WHEN** a pinned engine declares a code the vocabulary does not name
- **THEN** the wrapper's own test fails and names the codes it found

#### Scenario: A replay refusal that changed nothing is told apart
- **WHEN** a journal is replayed onto a document that is not the snapshot it
  continues from
- **THEN** the refusal is reported as its own kind rather than as an unknown
  code, because it is the one replay refusal that leaves the document
  byte-identical and is therefore the one worth retrying with a different
  document

### Requirement: The full validation report is available, not only two bits of it
The wrapper SHALL expose the engine's whole mesh validation pass — the counts
as well as the booleans — and not only the watertight and manifold flags.

A caller told that a mesh is not manifold and not told **by how much** cannot
decide what to do about it: a handful of pinched edges in a large mesh and
thousands of them are different outcomes that the two-bit call reports
identically.

#### Scenario: A caller asks what is wrong rather than whether
- **WHEN** a mesh is validated
- **THEN** the number of non-manifold edges, boundary edges, degenerate and
  sliver triangles, and the Euler characteristic are available alongside the
  flags

### Requirement: A drag reports the region it invalidated
The wrapper SHALL offer the drag entry point that reports **where the gesture
reached**, and the host SHALL use it in place of reconstructing a region from
the brush size and the distance travelled.

A reconstruction cannot know what a layer fold above the layer can move, nor
what layers sharing an instanced edit list also change — both of which are
*under*-invalidation, which leaves stale surface on screen with nothing to
correct it.

The buffer MAY be sized from the layer's symmetry and re-asked on refusal,
because a buffer too small is refused before the first edit is recorded and
there is therefore no half-applied drag to unpick.

#### Scenario: A short buffer is refused rather than half-applied
- **WHEN** a drag is issued with a region buffer smaller than the gesture needs
- **THEN** the call is refused, the document is unchanged, and the count needed
  is reported

### Requirement: A layer's mirror is read rather than remembered
The host SHALL ask the engine what mirror a layer carries rather than keeping
its own account of what it last set.

A history step moves the engine's symmetry without telling the host, so a cache
can only be abandoned when it *might* be wrong — which costs an unnecessary
write on the next stroke and, where the answer is used to decide what to
re-fill, an unknown mirror that must be treated as all three axes.

#### Scenario: A layer that cannot carry a mirror
- **WHEN** the engine declines to answer for a layer whose representation
  cannot express a mirror
- **THEN** the host falls back to the safe over-invalidating answer rather than
  reading a refusal as "no mirror"
