# Validation in progress

Engine pin remains v0.113.0 (`260b7797`). Native tests use the RTX 5060 Vulkan renderer with CPU field evaluation.

## Regression

The fresh-surface mask release test was run against production sources at `2997c38` (which already include measured mask uploads). It fails because release uploads 10,982,672 bytes, taking 89.757 ms in that run. With the settlement provenance check, the same fresh-surface test passes with zero release uploads. The existing sequence after a field edit and undo retains old geometry from separate requests and still requires a nonzero release upload; the test explicitly preserves that case.

Three renderer tests pass: the existing six-dab triangle-union comparison, initial/partial/compaction/full/empty state transitions, and live Move preview commit. The committed-epoch surface is bit-identical to a separate full rebuild across complete vertex attributes. A further unit regression verifies that empty bookkeeping entries do not keep old triangle ownership alive.

## Broader checks

All eight selected targets pass: library tests (65 at the broad run, plus the subsequently added empty-entry regression), main unit tests (28), native MCP E2E (3), lod_switching (6), sculpt_latency (4), settle_needed (3), visual_brushes (2), visual_incremental (3). The informational pruning benchmark remains intentionally ignored in the normal suite and was run previously. No native adapter skip occurred.

New functions stay below the 12-point Clippy complexity threshold. Formatting and all 45 strict OpenSpec items pass.

## Pending

- Final warning-denied lint and native release measurements against `2997c38`.
- Final PR documentation and platform CI.
- Remaining genuine brush work is not claimed to meet a universal 16 ms budget.
