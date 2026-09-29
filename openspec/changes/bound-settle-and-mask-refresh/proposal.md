# Bound settle and mask refresh work

Issue #175's first slice stopped whole-layer uploads on ordinary settles, cached capture targets, and made the settle ledger measurable. A release still hashes every retained triangle to remove duplicates, and a local mask stroke still samples every displayed vertex. Both costs grow with the document even when the edit is small.

## Change

- Retain the bricks replaced during a gesture until release, then prune duplicate triangles only among those bricks and their immediate neighbours.
- Carry a mask stroke's world-space influence box to the viewport. Refresh mask attributes only on intersecting bricks. Invert, clear, history, and layer changes continue to refresh the full surface.
- Measure release overhead separately from engine meshing on three worked scene sizes.

## Impact

The viewport's mask and settled geometry must agree with the full refresh and rebuild bit for bit. The normal edit path should scale with the changed region; explicit full rebuilds keep their complete duplicate pass.
