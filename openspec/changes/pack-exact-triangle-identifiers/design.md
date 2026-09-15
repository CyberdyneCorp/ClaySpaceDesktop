# Design

The existing vertex table assigns unique IDs to complete ten-word vertex bit patterns. Triangle equality is equality of the sorted three IDs; packing those IDs therefore changes storage, not the equality relation. Three non-overlapping 32-bit lanes fit in u128. The caller selects packing only if the sum of input vertex counts is at most u32::MAX, which bounds every assigned ID. Larger inputs use the existing [usize; 3] key. Keep this choice outside the hot loop through a generic key constructor; do not truncate IDs without proving the bound.

For each sorted brick, iterate complete triangles in original order. Retained triangles move only when an earlier duplicate created a gap; truncate once at the end. This preserves the former behavior for incomplete trailing indices and avoids allocating a second index vector. The later release compaction still reclaims unreferenced vertices and spare capacity.

Test exact output against the original independent reference, including collisions, seeds, duplicated owners, permutations, float payloads and trailing indices. Verify packed lane extremes and the wide fallback selection without allocating billions of vertices. Force both key representations on ordinary fixtures for comparison. Production timing must confirm the prototype gain.
