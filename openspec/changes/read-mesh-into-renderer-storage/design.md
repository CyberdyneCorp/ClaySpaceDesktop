# Design

Vertex already has repr(C), Pod and Zeroable and states the exact forty-byte renderer layout. Initialize its final vector with zero positions/normals/mask and white color. The existing engine copy writes positions/normals and optional colors directly through bytemuck::cast_slice_mut. No uninitialized storage or unsafe cast is needed.

Keep the original index copy and empty-mesh shortcut. The original reader decodes little-endian words; on big-endian targets normalize only the copied fields afterward to preserve that behavior, leaving default white color untouched. Supported local/platform validation is little-endian; do not claim big-endian runtime validation.

Verify complete bits against the original byte-decoding reference for colored/uncolored and empty real meshes, and preserve absent-normal errors. Measure allocations and production timing separately from native/rendered correctness and full brush latency.
