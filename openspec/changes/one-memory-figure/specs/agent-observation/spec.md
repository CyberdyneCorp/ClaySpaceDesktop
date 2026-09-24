## ADDED Requirements

### Requirement: The memory an agent reads is what the document is costing
`state.memory.in_use_bytes` SHALL be the figure in use: the engine's report with
the surfaces folded in, the brick cache, and what the application holds to draw
the document. It SHALL NOT be the engine's figure alone, which leaves out
everything the application allocates and read 10x to 2,000x low against the
process in the audited sessions.

`state.memory.parts` SHALL list where it is, including the brick cache and a
host-owned drawing part with its own parts: the stored geometry, the buffers,
the staging and the render targets. `cache_bytes` SHALL be the brick cache on
its own, beside the budget that bounds it. Where the platform lets the
application read what the operating system charges the process, the state SHALL
carry it beside the figure, as the figure the ledger is checked against rather
than as part of it.

#### Scenario: The drawing is a part of its own
- **WHEN** an agent reads memory with a surface drawn in the viewport
- **THEN** the parts include the drawing and its geometry, buffers, staging and
  targets, and `in_use_bytes` includes them

#### Scenario: The budget is compared with what it bounds
- **WHEN** an agent reads memory
- **THEN** `budget_bytes` is the brick cache's budget and `cache_bytes` is the
  brick cache, both beside the whole
