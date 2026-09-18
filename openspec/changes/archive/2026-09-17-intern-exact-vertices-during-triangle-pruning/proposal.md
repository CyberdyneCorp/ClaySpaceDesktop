## Why

ClayCore #531 remains affected by host release work. Native profiling of a clean sphere attributes 54–67 ms to exact duplicate pruning, versus approximately 2.5 ms for geometry placement/upload. Each triangle hashes three complete ten-component vertices. A prototype interns exact vertices first and reduces pruning to 11–14 ms without weakening equality.

## What Changes

Intern each complete bit-exact vertex into a temporary ID, then deduplicate triangles using sorted triples of IDs. Preserve sorted brick traversal, first occurrence, triangle/index order, and vertex arrays. Reserve temporary tables once. Retain full hash-table equality checks.

## Impact

Surface layout/compaction only; no engine pin, mesh field, normals, document data, or GPU format changes. Add exact reference regressions and run rendered compaction tests. This complements the pending-work measurement fix and does not remove the full release rebuild.
