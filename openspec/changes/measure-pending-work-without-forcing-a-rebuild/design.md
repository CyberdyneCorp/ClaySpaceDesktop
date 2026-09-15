## Context and evidence

The engine PR CyberdyneCorp/ClayCore#602 reduces exact field and meshing costs, but the live Standard measure still reports 123.8/100.6/105.4 ms for begin/continue/end while recorded incremental engine work is around 1–2 ms. The host meter adds an unprofiled full rebuild. A stroke end additionally leaves settle_owed set, allowing another rebuild on the following frame.

## Design

One helper drains the geometry the application actually owes. Synchronize only when SculptViewModel reports pending remeshing; this also avoids forcing a reduced-detail surface through the full-resolution sync path when nothing changed. Then flush an owed settle through the existing live-gesture guard. The flag is cleared by the existing flush before the work runs, so measure does not leave the same debt for the next frame.

Measure times application dispatch plus this helper. Wait uses the same helper, reports deferred settle debt as outstanding, and ends when quiet, its budget expires, or a pass makes no progress. A live gesture prevents its deferred settle; wait must return that outstanding work rather than spinning on the interface thread that would need to close the gesture.

Reuse the GPU's existing cumulative uploaded-byte counter. Read before/after each synchronous operation and return the saturating delta in Measured and Settled. These operations run between frames on the interface thread, so frame counter resets cannot interleave. This is diagnostic data, not a new renderer path. It makes an idle operation's lack of GPU writes testable without a machine-dependent time threshold.

## Verification

First add the counter and a real-process MCP regression while retaining the old settlement behavior; it must fail because idle measure/wait upload a full mesh. Then change the pending-work path and rerun. Exercise a real stroke as well: edits must still upload geometry, one gesture remains one undo step, capture must see the updated surface, and a following idle wait performs no upload. Test deferred work that cannot complete while a live gesture is open.

Repeat the clean-sphere 13-tool live measure sweep with the same engine revision and brush samples as the reproduction. Report command work and remaining expensive operations honestly; do not call every brush interactive merely because the artificial floor disappears.
