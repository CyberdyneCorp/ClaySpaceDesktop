## ADDED Requirements

### Requirement: The report says which side of the boundary a re-mesh's time went to
Today the report names an operation and a total. A total spanning an engine
call and the application's own work around it cannot be acted on by either
party: neither can tell from it whether the cost was theirs.

The diagnostics report SHALL attribute the cost of a re-mesh across the phases
it is made of, naming for each whether it is the engine's work or the
application's. The engine's meshing call and the application's copy, split and
upload SHALL be separate figures.

The report SHALL carry the same attribution for the engine's edit — the call
that applies a stroke and refills the bricks it dirtied — which is a distinct
call from meshing and is not measured today at all.

#### Scenario: A stall is reported with its breakdown
- **WHEN** a person copies the diagnostics report after a re-mesh has stalled
- **THEN** the report carries, beside the operation's total, how those
  milliseconds divided between the engine's calls and the application's work

#### Scenario: The engine's edit has a figure
- **WHEN** strokes have been applied this session
- **THEN** the report carries what the engine's stroke application and brick
  refill cost, separately from what meshing cost

### Requirement: A phase is reported as a distribution
A single figure for a phase invites a reader to treat one sample as the
answer, and a mean hides the tail that a sculptor is actually complaining
about.

For each phase it reports, the report SHALL carry the sample count, the median
and the worst observed value. A phase with no samples SHALL be reported as
having none rather than as costing zero.

#### Scenario: A phase that never ran says so
- **WHEN** the session has applied no stroke
- **THEN** the stroke section states that no samples were taken, and reports no
  durations

### Requirement: The report carries the evidence the refill routing was decided on
The application routes each brick refill to the CPU or to the accelerated
backend on measurements it takes at runtime, and those measurements are
currently visible to nothing but a test. They are the evidence behind the
finding that an accelerated backend can be several times slower than the CPU on
a given machine, which is a fact about the engine that only this application is
positioned to observe.

The diagnostics report SHALL carry the measured cost per brick of a refill on
each backend the routing considered, and SHALL state where a backend has not
yet been measured rather than reporting it as costing nothing.

#### Scenario: Both backends have been measured
- **WHEN** the routing has timed a refill on the CPU and on the accelerated
  backend
- **THEN** the report carries both costs per brick

#### Scenario: The routing is still running on its constant
- **WHEN** one of the backends has not yet been timed
- **THEN** the report says that backend has not been measured, and does not
  report a cost for it
