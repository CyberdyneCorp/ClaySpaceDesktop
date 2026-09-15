## Design

An exact vertex is the existing ten f32 bit patterns: position, normal, color and mask. A HashMap assigns equal keys equal temporary usize IDs; collisions still compare all bits. Sorted triples of these IDs have exactly the same equivalence classes as sorted triples of vertex keys. IDs never escape the pruning call. Using usize avoids narrowing a potentially large store.

Traverse brick keys in their existing sorted order and triangles in authored order, keeping the first occurrence. Preserve each vertex vector and surviving index sequence. Empty geometry does not contribute to the tables. Keep the original algorithm as a test-only reference.

Memory trades the large 120-byte triangle key for a 3-usize triangle key plus one 40-byte vertex key/ID entry per distinct vertex. Tables are temporary, preallocated from stored triangle and vertex counts, and dropped after layout. Measure cases with many shared vertices and cases with little reuse rather than assuming all meshes improve.

## Verification

Compare exact per-key vertices and indices against the original algorithm, including permutations, empty stores, duplicates within/across keys, all ten attributes, signed zero and NaN payloads. Run existing visual_holes, visual_incremental, lod_switching and sculpt_latency tests as applicable. Repeat live same-configuration measurements and report release separately from drag. Check Clippy complexity, formatting, strict OpenSpec and CI.

## Scratch-table hashing follow-up

Use `ahash::RandomState` for the temporary exact vertex-ID map and triangle-ID set. Add an explicit application dependency on the already locked ahash 0.8.12 package. Each pruning call creates a randomized state; full key equality still resolves collisions. Sorted brick traversal and first occurrence determine output independently of table iteration or seeds. Keep other maps unchanged.

A private helper accepts a build-hasher to verify multiple seeds and a deliberately constant hasher against the original full-vertex reference. Test exact attributes, duplicate permutations and distinct triangles under collisions. Compare isolated shared-vertex and triangle-soup workloads, then measure actual releases before claiming application speedups.
