## ADDED Requirements

### Requirement: A millisecond budget is a verdict only where it measures the code
A test that holds a frame or a gesture to a budget in milliseconds SHALL
assert that budget only in an optimised build on a machine that is not a
hosted runner. In a debug build the same work measures the build profile, and
on a hosted runner it measures the runner: the first drag frame of the mesh
reference reads 4.5 ms in release on an Apple M3 Pro, 17–46 ms in release on a
`macos-14` runner and 95–142 ms in debug there, for the same work. Everywhere
the budget is not asserted the test SHALL still run the work, print the figure
with the reason it is not a verdict, and assert the properties that do not
depend on the machine — what was re-evaluated, what the document holds, what
was drawn. The budget itself SHALL NOT be loosened to fit a runner.

#### Scenario: A millisecond budget is a verdict in release off a hosted runner
- **WHEN** a budgeted test runs in an optimised build with `CI` unset
- **THEN** a frame over its budget fails the test

#### Scenario: A millisecond budget is a figure in a debug build or on a hosted runner
- **WHEN** a budgeted test runs in a debug build, or with `CI` set
- **THEN** the frame's cost is printed with the reason it is not asserted, and
  the test still fails on a brick re-evaluated, a document moved or a surface
  not drawn where it should be
