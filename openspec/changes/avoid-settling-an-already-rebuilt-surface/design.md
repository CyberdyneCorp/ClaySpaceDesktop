## Invariant

A store built by one meshing request has one consistent triangle ownership assignment. Merging subsequent partial requests can leave boundary copies under different keys, so it conservatively requires release settlement even if no duplicate was observed. Exact duplicate pruning cannot clear this state because different boundary copies may carry different shading attributes.

Track a needs-settle bit. On a successful remesh replacing all stored triangles, or the full rebuild path, clear it. On a partial remesh retaining old triangles under unreplaced keys, set it. Empty bookkeeping entries do not count as retained geometry. This also recognizes undo requests that replace the entire surface despite using the dirty-key path. An empty full rebuild also clears it. Attribute-only mask/cage changes and layout compaction preserve it. The initial sync uses a partial-request argument but starts with an empty store; it must count as one request, not manufacture initial settlement debt.

Keep the existing EndStroke debt and live-gesture guard. When flushing allowed debt, clear the host flag and rebuild only if the geometry still needs settlement. If a preview-to-document epoch change already rebuilt it, that rebuild satisfied the debt. Do not bypass explicit rebuild/settle calls.

## Verification

First require a measured mask release to perform zero uploads after its measured attribute update; the old unconditional settlement must fail. Verify initial single-request sync, partial updates requiring settlement, explicit full/empty rebuilds clearing it, and compaction retaining it. Retain rendered comparisons and live-gesture guard tests. Measure both mask-only and epoch-changing releases, reporting real remaining work. No universal 16 ms claim.
