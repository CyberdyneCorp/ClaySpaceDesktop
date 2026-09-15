## Why

ClayCore #531 still pays unnecessary release work. The host owes a full settlement after every SDF stroke, including mask-only edits and tools whose preview-to-document epoch change already triggered a full rebuild during synchronization. These paths need no second rebuild of the same surface.

## What Changes

Track whether stored triangles combine separate partial meshing requests. Preserve settlement for that conservative case, but consume deferred debt without rebuilding a surface already produced by one request. Preserve the live-gesture guard, complete mask rendering, and retain explicit rebuild operations.

## Impact

SurfaceGeometry state and host deferred settlement only. No engine pin or field/shading approximation changes. Add native zero-upload release regression and renderer state/geometry tests.
