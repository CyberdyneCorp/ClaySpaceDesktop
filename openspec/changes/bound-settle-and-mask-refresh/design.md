# Design

## Release compaction

`SurfaceGeometry::touched` is consumed by each upload, so a second set remembers replaced brick keys through the gesture. Release expands those keys by one brick in each axis and compacts only the resulting subset. The engine attributes triangles touching a boundary to the lowest requested key containing a corner; a duplicate created by a partial request can therefore only be in this one-brick neighbourhood. Explicit full relayouts still use the complete pass.

## Mask refresh

The document records pending mask change scope beside `mask_revision`. A brush stroke contributes the union of its sample positions expanded by brush radius and two mask cells for interpolation. Consecutive strokes union their boxes until the viewport consumes them. Operations that may affect the entire mask mark a full refresh. The viewport intersects that box with retained key vertex bounds before sampling; an empty mask with no drawn weights exits without touching any key.

The pending scope is consumed only when the viewport refreshes. A failed mask operation does not advance the revision.

## Measurement

The settle benchmark times application-side compaction and upload separately from engine meshing, and reports the scene's triangle count next to the overhead. Three radii of the same worked field target about 50,000, 300,000 and 1,000,000 triangles. A 30 ms overhead gate applies to both fixtures under 300,000 triangles on every backend.
