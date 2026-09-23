# Design

## Evidence

Six live full-preview meshes in the current Linux fixture spend median 11.930 ms in per-brick splitting, versus 30.562 ms in engine meshing and 3.218 ms in readback/conversion. Each brick hashes global indices for every triangle corner, then populates local vertices by the resulting assigned indices.

## Candidate

Allocate a `Vec<Option<u32>>` indexed by the returned mesh's global vertex index once per split. For each brick, append its vertex on first encounter and reuse its local index on later encounters. Record touched global indices and clear only those entries between bricks. This keeps first-encounter numbering and triangle traversal exactly as before while avoiding repeated hashing.

The dense table is temporary and proportional to the returned vertex count (approximately eight bytes per vertex), not the document's historical size. The touched list is reusable and bounded by the largest processed brick. Measure this memory increase alongside the speed change. Do not retain an unbounded cross-frame cache.

## Invalid input and empty ranges

The current splitter assigns local indices even to out-of-range global indices and leaves their vertex values at the existing default. Detect such a brick and use the original hash-map path for it. Do not silently drop triangles, panic on an unchecked index, or invent a different placeholder. Absent/empty ranges still clear their existing per-key geometry.

## Verification

Compare complete vertex bits and local indices with an independent copy of the original algorithm. Cover repeated corners, shared vertices across consecutive bricks, reversed processing order, empty ranges, unusual float payloads, invalid indices and reused destination buffers. Use realistic full and subset mesh ranges for profiling; synthetic correctness checks alone cannot establish a speedup. Re-run rendered and settlement regressions before reporting application results.
