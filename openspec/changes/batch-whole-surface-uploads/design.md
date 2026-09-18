# Design

For mapped compaction, allocate fresh slots in the same key order, skipping empty geometry. Retain borrowed brick spans and the used vertex/index prefix lengths. Prepare into a temporary slot map before reserving GPU buffers; if every brick cannot be placed, refuse the layout and preserve the existing mesh and slot map. Retain device-budget checks and incremental per-key writes. Full rebuilds retain their original reservation and per-brick patch sequence.

Fill wgpu's mapped upload buffers directly through renderer callbacks. Copy live vertex bytes and zero only gaps between spans. Rebase live indices and fill slot tails with the owning vertex base (degenerate triangles). Compute bounds only from live vertices. Retain the slot map for subsequent isolated patches; omit the last unused vertex tail from the upload.

The callbacks expose byte slices, with no assumption about u32 alignment. Writers do not read mapped contents and initialize every transmitted byte. Empty uploads skip the callback. The renderer knows nothing about brick keys or slots.

## Tradeoffs and evidence
The initial CPU-array prototype remains in the validation history and as a test-only reference. Its diagnostic run under load showed substantial preparation time and roughly 21 MiB of temporary array capacity. The revised plan stores only one borrowed span per nonempty brick; wgpu still owns the required upload storage. Vertex gaps still add about 23% to the measured whole-layout payload, so fewer calls alone are not acceptance evidence.

Validate byte-for-byte equivalence against the CPU-array reference, including nonzero-prefilled, deliberately unaligned destination slices, NaN attribute payloads, and signed zeros. Verify real GPU pixel equality, byte accounting, empty uploads, sculpting, and settlement.

The informational in-process benchmark compares original per-brick writes, CPU-array batching and direct batching on the same real SDF sphere. It rotates order, drains GPU work between samples, warms three rounds and records thirty samples per variant. It isolates upload CPU time and excludes meshing, pruning, transfer completion and presentation; final acceptance still needs paired whole-application measurements.

## Restrict batching to compaction
The completed ten-pair application comparison found a consistent Smooth-start regression with mapped uploads applied to every layout (49.779 to 53.400 ms, slower in all ten pairs), despite the isolated upload improvement. Ordinary SDF releases improved. Therefore use mapped uploads only after `compact_release_geometry` in `settle_after_edit`; full rebuilds, preview initialization, and ordinary relayout retain the original per-brick writes. This dispatch follows the geometry operation, not brush names. The final ten-pair comparison retains ordinary-release gains and removes the consistent Smooth-start regression; see validation.md for complete results and limits.
