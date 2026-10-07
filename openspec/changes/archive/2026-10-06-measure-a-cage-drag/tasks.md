## 1. Benchmark

- [x] 1.1 `groups/cage.rs`: `cage.drag_3`, `cage.drag_8`, `cage.drag_32` one-shot frame figures with the 16 ms budget attached.
- [x] 1.2 `cage.scaling` (32³ over 3³, budget 3×) and `cage.memory` (after a 100-frame drag over before, budget 1.2×).
- [x] 1.3 Registered after `op` in `measure_everything`. The figures are new, so the committed baselines still compare.

## 2. Verification

- [x] 2.1 `lattice.rs`: `a_long_drag_leaves_the_documents_memory_where_it_found_it`.
- [x] 2.2 `just bench-only cage` run and its figures recorded in the proposal.
