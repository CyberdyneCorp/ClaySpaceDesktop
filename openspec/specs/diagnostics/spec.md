# diagnostics Specification

## Purpose
What the application can say about itself when something is slow or large: where
a document's memory has gone, which side of the engine boundary a re-mesh spent
its time on, and the evidence behind the decisions the maintenance path took.
## Requirements
### Requirement: The report says where the document's memory is, not only how much
The diagnostics report SHALL carry the document's memory broken down by what
releasing it would cost: the user's work, which is never released; what
reconstructs identically, whose release costs only a stall; and undo depth,
which is the application's own policy. It SHALL carry the total beside them,
and SHALL NOT carry the total alone.

The breakdown SHALL be the engine's own classification of its own figures. The
application SHALL NOT re-derive it, so that a category added to the engine and
left unclassified cannot make the parts and the total disagree.

The figures SHALL include every surface the application holds beside the
document. A sculpting session is an owning handle the host keeps next to its
document rather than inside it, so the engine cannot walk one and reports it as
nothing; the application SHALL ask each session what it costs and hand the
result back to the engine rather than publish a figure that omits it.

The report SHALL state how many surfaces were asked, as well as what they came
to, so that a surface figure of zero can be read as *there are none* rather
than as *nobody asked*.

#### Scenario: The breakdown names which part
- **WHEN** a person copies the diagnostics report
- **THEN** it states what is the user's work, what is rebuildable and what is
  undo depth, each as a figure, beside the total

#### Scenario: A sculpting session is in the figure
- **WHEN** the document holds a mesh subtool that has been sculpted
- **THEN** the reported total includes what that session costs, and is larger
  than what the engine reports for the document alone by exactly that amount

#### Scenario: A document holding no surface still says so
- **WHEN** the document holds no sculpting session
- **THEN** the report states that zero surfaces were asked, and the figures are
  the ones the engine reports for the document alone

### Requirement: The report says whether an agent could have been driving
The diagnostics report SHALL carry the state of the agent-facing server:
whether it is listening, on what address, whether a client is connected, and
how many of this session's commands arrived from one.

This is in the report for the reason the engine revision and the container
minor are: a defect report that does not say a second party was driving the
application is a report whose steps cannot be trusted to be the whole of what
happened. "It moved on its own" and "an agent applied forty strokes" are the
same symptom with different causes, and only one of them is a defect in this
application.

The report SHALL NOT carry the connection secret. A report is pasted into
issues and chat windows, and a secret that reaches one of those is a session
anyone reading it can drive.

#### Scenario: A report from a driven session says so
- **WHEN** the diagnostics report is produced during a session an agent has
  been acting on
- **THEN** it says the server is listening, and how many commands came from an
  agent

#### Scenario: A report from an untouched session says that too
- **WHEN** the report is produced with the server stopped or no client ever
  connected
- **THEN** it says so explicitly rather than omitting the section

#### Scenario: The secret is not in the report
- **WHEN** the report is produced with the server listening
- **THEN** the connection secret does not appear in it

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

