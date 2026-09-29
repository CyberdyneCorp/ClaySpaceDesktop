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

### Chunked drawing (#209)

**One sculptor for the surface's life.** The engine's chunk table and dirty
set belong to the sculptor, and a sculptor borrows its surface, so the
application held a surface and made a sculptor per stroke segment — paying the
index build every segment (measured 117 ms at 100,352 triangles, 1.50 s at
1,002,528) and starting each with an empty dirty set. `claycore` gains
`DynamicSession`, which owns the boxed surface and its one sculptor, drops them
in that order, and lends the sculptor only through a closure whose lifetime the
caller cannot name, so two sessions cannot exchange sculptors. A topology
delta (#210) is captured and replayed through that same sculptor, so an undo,
a redo or a dragging verb's replay on a closed surface marks the chunks it
touched and is patched like a stroke; restoring bytes (the snapshot record an
open surface keeps) makes a new session and lays the region out again.

**A mirror of the chunks, drained from the dirty set** (`chunked::ChunkMirror`).
The first drawing, and any new chunk table, copies every chunk; afterwards
`dirty_chunks` → `copy_chunk_into` → `clear_dirty`. Chunks arrive as unwelded
triangles and are welded here on exact position and normal, which keeps the
drawn buffer near a welded export's size (70 MB at 1M triangles laid out with
headroom, against 216 MB unwelded). A chunk's change is *Geometry* when its
welded indices are identical to the last copy and *Topology* otherwise, read
off the data rather than trusted from a flag, so a patch never sends indices it
does not need and never skips ones it does. Buffers and scratch are kept, so a
stroke that does not grow the surface allocates nothing. Changes merge per
chunk until the drawing takes them, so draining for another reason (the bounds
after a stroke) loses nothing.

**Chunk slots in the carried buffer** (`chunked::Region`). The carried layers
are one buffer; a Dynamic layer's part of it is laid out as a slot per chunk
with the brick path's headroom (`slots::SlotMap`, moved to the engine crate and
made generic over its key), plus spare room at the end of an eighth, never less
than 16,384 triangles. Padding is degenerate triangles on a real vertex of the
surface, and the census leaves it out. A patch rewrites a moved chunk's
vertices, a re-cut chunk's vertices and whole index slot, and blanks a slot a
chunk left or emptied. `ClayDocument::carried_patch(build)` offers the runs
only while nothing it cannot follow has moved — `mesh_revision` is split into a
layout revision and the chunked surfaces' own revisions — and only against the
build the caller uploaded, since any other caller of `visible_mesh_geometry`
lays the regions out again. Otherwise it answers `None` and the caller builds
the buffer whole, as before. A Dynamic stroke no longer bumps
`live_generation`, and banking an adaptive gesture no longer counts as a mesh
history change, so neither forces a rebuild.

**The renderer writes the runs in place** (`Renderer::patch_mesh_layers`),
widening the patched subtool's culling box, and declines — writing nothing —
while the polyframe is on or no copy of the index list is kept for it, because
its lines are derived from the whole index list.

**Index rebuilds follow the engine's word.** After a gesture banks, if
`index_quality().wants_rebuild`, an `IndexRebuild` is queued with the surface's
last measured rebuild cost; the between-strokes drain decides. A rebuild
renumbers chunks, so it counts as a new generation and the next upload lays the
surface out afresh.

**Measured** (flat sheets of one density, one Draw dab): 271 KB at 100,352
triangles and 327 KB at 1,002,528 against 7.9 MB and 70.3 MB for the whole
region (debug test); in the release benchmark 175 KB and 194 KB a dab, the
drawing half 0.29 ms mean at both sizes, the whole dab of an open stroke 11.1 ms
at 100k (held to 16 ms) and 15.3 ms at 1M.

**Limits.** A surface carrying vertex colour is still copied whole when it
moves: neither `clay_dynamic_surface_copy_chunk` nor
`clay_surface_view_copy_chunk` copies an attribute, so a chunked colour surface
would lose its paint — an engine gap. A full rebuild of a Dynamic region costs
more than the old whole copy (headroom and seam duplicates: 7.9 MB against
about 3.3 MB at 100k), paid on layout changes rather than per dab. On a surface with an open
boundary the first segment of a gesture still serializes the whole surface as
its undo record (66 ms at 100k, 661 ms at 1M); a closed surface records the
engine's reversible delta instead (#210).
