# Design: DynamicSurface integration

## Wrapper and ownership

`claycore` owns the opaque DynamicSurface handle and safe wrappers for create
from mesh, edit, preflight to mesh, conversion, serialization, revision and
dirty-chunk copy calls. Rust types own all returned buffers. The pinned header
is the source for entry points, memory ownership and error codes. Wrapper
tests round-trip representative geometry and refuse invalid conversion.

## Model and persistence

`Representation::Dynamic` is a distinct document variant and serialized tag.
Its state includes the engine-owned surface and enough metadata to restore it
without flattening it into a fixed mesh. Loading an older document preserves
its existing representation; unknown tags are refused rather than guessed.
The capability table adds Dynamic bindings only for operations the wrapper
actually executes, with Layer explicitly absent and explained.

## Conversion

Mesh → Dynamic and Dynamic → Mesh are explicit commands with a preview of
expected cost and loss. Dynamic → Mesh runs engine preflight first, showing
the result or refusal before replacement. A successful conversion is one
history entry, updates the layer identity and selection coherently, and does
not silently run to satisfy a brush request. A refused preflight changes
nothing.

## History

Dynamic edits that change topology store the engine's reversible state, or a
bounded snapshot when a reversible delta is unavailable. The existing
document sequence orders them with all other commands. Undo and redo restore
connectivity, geometry and attributes and invalidate affected viewport
chunks. This does not create a second history stack or compare undo depths.

## Drawing

Keep a stable GPU allocation per chunk where possible. Track separate
topology, geometry and attribute revisions; a changed topology replaces that
chunk's index and vertex layout, a changed geometry updates positions and
normals, and an attribute-only change uploads only attributes. Deleted chunks
are removed. Camera movement alone does not cause a surface upload.
Correctness tests compare incremental drawing to a full rebuild after edits,
undo, redo and conversion; performance tests bound bytes uploaded to dirty
chunks rather than the entire surface.
