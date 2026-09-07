## ADDED Requirements

### Requirement: The cut entry points are wrapped where the unsafe lives
`clay_cut_create`, `clay_cut_polygon_from_open_curve` and
`clay_cut_polygon_from_curve` SHALL be wrapped in the `claycore` crate, which is
the only crate permitted `unsafe`, and SHALL be reached from above through safe
types.

The size-query pattern the two polygon calls use — a first call with a null
output pointer to learn the vertex count — SHALL be held inside the wrapper. A
caller above the bridge SHALL receive a vector and SHALL NOT be able to make
the second call with a buffer sized from a stale count.

The item `clay_cut_create` returns SHALL be owned by a type that frees it, so
that a refused placement cannot leak it.

#### Scenario: A cut outline is asked for
- **WHEN** a polygon is resolved from a drawn curve
- **THEN** the caller receives the vertices, having made no decision about
  buffer sizing

### Requirement: A refused frame is reported rather than corrected
A frame that is not orthonormal SHALL be reported as the engine reports it, and
SHALL NOT be squared up before the call.

The engine refuses one deliberately — the shape the sculptor saw was drawn in
the frame they think they have — and a bridge that quietly orthonormalises
would cut a shape nobody drew.

#### Scenario: A frame is degenerate
- **WHEN** a cut is asked for with a basis that is not orthonormal
- **THEN** the call is refused and the refusal reaches the interface, with no
  cut placed

### Requirement: A cut's placement carries the region it must pass through
The descriptor SHALL carry the bounds of the region being cut, and SHALL leave
the near and far extents at zero so the engine derives the sweep from them.

Both extents at zero is what the engine names as the caller's ordinary want; a
non-zero pair is a deliberate partial cut, which this tool does not offer. A
sweep sized by anything other than the region risks a cut that stops inside the
form and leaves a shelf.

#### Scenario: A form is cut through
- **WHEN** a Trim is placed on a subtool of any size
- **THEN** the cut passes entirely through it
