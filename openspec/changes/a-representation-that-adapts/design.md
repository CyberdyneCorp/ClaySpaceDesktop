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

### Pricing and identity (#208)

Both directions are priced by the engine before anything is built:
`clay_mesh_preflight_to_dynamic` over the active mesh layer going in (bound
as `Document::preflight_mesh_layer_to_dynamic`, which borrows the layer's
mesh and builds nothing) and `clay_dynamic_surface_preflight_to_mesh` coming
out. The engine is asked with no budget and answers with figures; the verdict
is the document's, peak on top of what the document already holds against a
surface budget that defaults to the hierarchy's 2 GB and that a host can
lower. A refusal names the direction, the peak, the held figure and the
limit, and runs before the undo group opens, so it changes nothing. The panel
reads the same preflight each frame (about a microsecond) and shows the peak
beside the budget; the held figure is read only when the crossing runs,
because the ledger walks every layer.

The crossings are exact, so the result carries the source's transform and
visibility inside the crossing's undo group. A freeze back to a mesh strips
the suffix the crossing in added, so an in-place round trip gives the name
back; a crossing that adds a layer keeps a unique derived name. The default
stays *add a layer*, keeping the original; measured on a 180,000-triangle
sheet in release, in place costs the same as beside (about 170 ms in and
85 ms out either way), so the audit's in-place premium belongs to the field
crossings and does not decide this default. Colour and UVs cross as corner
attributes; quads do not survive, and the panel says so before the crossing.

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
