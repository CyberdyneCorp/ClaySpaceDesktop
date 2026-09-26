# Tasks

- [x] Detect and close through-holes into a hollow, with unit tests for holes,
      sealed hollows, rings, grooves and wide openings.
- [x] Report found, closed, remaining and cells added for both repairs, and make
      a repair that adds nothing a no-op.
- [x] Bracket Close Holes as one undo step.
- [x] Show the outcome in the repair panel and announce it on the agent remark
      channel.
- [x] Skip the whole-surface settle after a crossing that leaves the field
      unchanged, with engine regression tests for both directions.
- [x] Document the repair counts and through-hole closing in `docs/features.md`;
      its grid display wording already agrees that Suave is the default.
- [ ] Measure and budget the grid-to-field converter at 5k, 23k and 100k cells,
      and move it off the interface thread (follow-up).
- [ ] Make grid mask extrude one undo step with visible geometry, or refuse it
      on grids (follow-up).
