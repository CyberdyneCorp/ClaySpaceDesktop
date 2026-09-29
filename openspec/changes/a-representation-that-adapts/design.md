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

### The topology delta (#210)

`claycore` binds the engine's record (`clay_dynamic_delta_*`, ABI 0.118.0)
and the recorded stroke (`clay_dynamic_sculptor_apply_stroke_recorded`, ABI
0.120.0) as `DynamicDelta` and `DynamicSculptor::apply_stroke_recorded`. The
recorded call rather than a host loop of recorded stamps, because the loop is
a different stroke for Grab and Snakehook.

A gesture opens one record on its first segment. Every segment and every
mirror is captured into it — a record whose end is the current surface is
continued — so one gesture is one `GestureRecord::Adaptive` in the carried
stack, stamped by the same monotonic sequence as every other entry (#151).
The record is symmetric, as `MeshDeltas` is: undo reverts it, redo applies it,
and the same record travels between the stacks. Replay goes through a fresh
sculptor, whose index follows the replay; no index rebuild is called. A
dragging verb reverts and clears the record before laying the gesture down
again from its anchor. Cancel is the ViewModel's floor rule: the gesture is
banked and the one entry above the floor is reverted.

The engine's limits and how the history holds to them:

- **Last in, first out.** The ordered stack is already LIFO; an out-of-order
  or foreign record is refused by the engine before it writes anything.
- **A record never outlives its surface handle.** Nothing replaces an adaptive
  surface underneath a layer that holds delta records. An undone crossing
  keeps its `Adaptive` in the retired set, so its redo brings back the same
  handle and the stroke records above it still replay. An open document starts
  with an empty history.
- **Verified by digest, not bytes.** Tests compare a digest over the exported
  triangle indices and position bits and check `clay_dynamic_surface_validate`;
  the serialized surface is not byte-identical after an undo.

**Open boundaries keep the snapshot.** Measured on the pinned engine (ClayCore
v0.120.1): reverting a record whose stroke reached an open boundary restores
the exported triangles exactly but leaves a live boundary half-edge whose
`next` is a dead slot, and `clay_dynamic_surface_validate` fails. So the kind
of record is decided when a gesture opens: a closed surface
(`boundary_edges == 0`) records the delta, an open one the surface's bytes, as
before. A snapshot restore is a new surface identity, so the two kinds are
not meant to share a layer: a boundary is not expected to appear or vanish
under the brush, and if one did, the engine refuses the stranded delta with
`CLAY_ERROR_SNAPSHOT_MISMATCH` before writing anything, so that undo step is
lost rather than the surface corrupted.
`claycore`'s `a_revert_at_an_open_boundary_leaves_a_dead_next` pins the
defect and fails when the engine fixes it, which is the signal to drop the
fallback.

**Memory.** A delta weighs its `resident_bytes` and a snapshot its length;
both are spent against the carried history's 256 MB budget
(`multires::HISTORY_BYTES`), dropped from the oldest end, and both are
reported in the diagnostics `adaptive surfaces` line as undo steps and bytes,
since `clay_dynamic_sculptor_memory_ledger` counts no record. Measured for one
Draw stroke on a closed ball in a release build: 1,917,296 bytes resident for the delta against
a 1,564,320-byte snapshot at 6,016 faces, and 3,143,008 against 25,426,080 at
97,792 faces. The delta follows what the stroke reached; the snapshot follows
the surface.

## Drawing

Keep a stable GPU allocation per chunk where possible. Track separate
topology, geometry and attribute revisions; a changed topology replaces that
chunk's index and vertex layout, a changed geometry updates positions and
normals, and an attribute-only change uploads only attributes. Deleted chunks
are removed. Camera movement alone does not cause a surface upload.
Correctness tests compare incremental drawing to a full rebuild after edits,
undo, redo and conversion; performance tests bound bytes uploaded to dirty
chunks rather than the entire surface.
