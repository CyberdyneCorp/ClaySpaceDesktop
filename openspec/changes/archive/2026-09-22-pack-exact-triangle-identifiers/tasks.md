- [x] Compare index-width, packed-key and in-place-compaction prototypes on exact output and timing.
- [x] Implement bounded packing with the wide fallback and stable in-place compaction.
- [x] Add boundary, fallback, exact-output and collision regressions.
- [x] Complete native/rendered tests, lint, complexity and strict OpenSpec checks.
- [x] Measure production memory and application latency; update PR evidence and preserve the full 16 ms goal.
- [ ] Complete platform CI and retain the full 16 ms goal for further work.
      — **superseded**: the code shipped in #137, later work builds on it, and every
      platform row has been green on main since
