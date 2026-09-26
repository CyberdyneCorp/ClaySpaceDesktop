## ADDED Requirements

### Requirement: Background jobs are outstanding until their result is placed
A retopology, UV layout, conform or texture bake running off the interface
thread SHALL be reported as outstanding work — by `wait`, by the settle a
capture performs and by the `jobs` state section — from the moment it starts
until its result has been collected, whichever path started it. `wait` SHALL
NOT report the session quiet while one runs.

A measured command that leaves such a job running SHALL say so in its answer,
so that the figure is read as the time to start the work rather than to finish
it.

#### Scenario: Retopology started through measure
- **WHEN** an agent measures `retopo` `run`
- **THEN** the answer lists the retopology as outstanding
- **AND** a following `wait` reports the session not quiet and names it
