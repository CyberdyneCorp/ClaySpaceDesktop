# Design

Use a private borrowed VertexBits wrapper with Hash and Eq defined over the same ten u32 float representations as vertex_key. Pointer identity and float equality are unsuitable: separate vertices may match and NaN/signed-zero bits must remain distinct.

Collect mutable map entries once and sort by brick key. Split each geometry into its vertices and indices; the interning table borrows only immutable vertex data while indices compact in place. Rust lifetimes enforce that no vertex storage moves or mutates until the pass ends. Release vertex compaction follows after all borrows end.

Retain capacity bounds, randomized hashing, packed triangle identifiers and wide fallback. The entry vector grows from copied keys to references, but hash buckets shrink from forty-byte keys to pointers. Measure net allocations and production timing, then full application timing, independently of builds.
