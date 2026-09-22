## Why

A sculpting session's process footprint climbed and never came back down. The
audited session reached **26 GB**, with about 13 GB of dirty graphics memory
spread over roughly 700 thousand regions of ~20 KB each, while the application
reported 13 MB in use. A second session's floor ratcheted 0.58 → 0.86 → 1.55 →
2.4 → 3.84 GB and stayed there.

Three things in `crates/clayspace-view/src/renderer/mod.rs` compounded:

- **Every layout allocated two new buffers.** `GpuMesh::reserve` created a
  fresh vertex buffer and a fresh index buffer unconditionally, dropping the
  pair it already had. A settle that re-lays a surface of an unchanged size
  asks for exactly the figures it asked for last time, so the allocation was
  pure churn — returned to the process only as fast as the driver felt like
  returning it.
- **Every key was patched with a write of its own.** In wgpu 24 each
  `Queue::write_buffer` takes a `StagingBuffer` of its own. The ~20 KB region
  size in the measurement matches those staging allocations rather than mesh
  buffers, which is what points at them as the dominant cost.
- **Nothing polled the device.** wgpu frees a staging buffer when the
  submission that consumed it is *seen* to have completed, and it is a poll
  that looks. `device.poll` appeared nowhere in a frame: only in a benchmark,
  in the offscreen capture path, and in the profiler when timestamps are being
  measured. So the staging buffers of a whole session accumulated.

`batch-whole-surface-uploads` already batched *compaction* into at most one
mapped write per buffer, and deliberately left the ordinary paths alone after
measuring that universal direct staging regressed whole-action latency. This
change is the part that one did not cover.

## What Changes

- **A reservation reuses buffers that already fit.** `GpuMesh::reserve` now
  grows only the buffer that has run out of room, geometrically and within the
  device's ceiling, and keeps the other. Both layout paths already write every
  slot they go on to draw, padding included, so a kept buffer's old contents
  are never drawn.
- **A settle writes contiguous ranges, not keys.** The incremental path places
  every touched key first and writes afterwards, merging spans that abut into a
  single upload. The keys a settle relocates are placed back to back, so their
  index spans — which cover their whole slot — become one write instead of one
  each. Full rebuilds keep their per-brick writes, as
  `batch-whole-surface-uploads` measured them to be faster.
- **The frame polls the device.** One non-blocking `Maintain::Poll` after the
  frame's submit, so completed submissions are seen and their staging memory is
  reclaimed within a bounded number of frames.
- **The device counts what it allocates and how many writes it took**, beside
  the bytes it already counted, so the two invariants above are assertable
  rather than argued about.

## Capabilities

### Modified Capabilities
- `incremental-surface-geometry`: buffer reuse across layouts, merged
  contiguous writes in a settle, and per-frame reclamation of upload staging.

## Impact

**Code**: `clayspace-view` (`gpu.rs`, `renderer/mod.rs`), `clayspace-app`
(`geometry.rs`, the settle's upload path). No engine pin, brush math or file
format change.

**What is not covered**: the memory *reporting* gap — the figure the
application shows against the footprint the process actually holds — is a
separate issue and is untouched here. The acceptance criterion asking for a
scripted thirty-minute session to return to within 20% of its starting
footprint needs a measurement harness that does not exist in this repository
yet; the gates here assert the mechanism instead, which is what a unit test can
see.

**Vertex merging in practice**: a vertex run covers only the live part of its
slot, so two vertex spans merge only where one slot's live part ends exactly
where the next begins. Extending a run over its slot's headroom would merge
more and transfer about 25% more bytes, which is the tradeoff
`batch-whole-surface-uploads` already measured against and declined. Index
spans, which cover their whole slot by construction, merge freely.

**Risk**: a kept buffer holds its old contents. Every path that reserves writes
the whole range it then draws — `upload_per_brick` patches each key's slot
including the degenerate tail, `LayoutUpload::write_vertices` fills the gaps
with zeros and `write_indices` covers every slot — so nothing stale is
reachable from the draw call. `a_rebuilt_layout_of_the_same_size_allocates_nothing`
and the existing incremental agreement tests fail if that stops being true.
