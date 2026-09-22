## MODIFIED Requirements

### Requirement: The status area reports document, memory and backend state
The status area SHALL display the current document name and modified state, the working unit, the memory in use against the configured budget, the active evaluation backend, and whether the application is listening for an agent.

The listening indicator SHALL say whether a client is currently connected, and
SHALL show when an agent last changed the document. A surface that moved while
nobody touched the window is otherwise a defect report with no cause in it.

The memory figures SHALL be read from the engine at most once a second rather
than once a frame, and a reading SHALL be at most a second behind what the
engine would answer now. Reading them is a walk of the whole brick cache, at a
cost proportional to the sculpture — on a worked document it was the largest
single thing on the idle main thread — and a meter beside a progress bar is
read by a person, not derived from.

A reading the engine refuses SHALL leave the figures as they were rather than
showing nothing in use: a cache that cannot answer has not thereby released its
memory.

#### Scenario: Memory reflects the engine's own accounting
- **WHEN** memory usage is displayed
- **THEN** the figures come from the engine's brick cache statistics and budget, not from an estimate maintained by the application

#### Scenario: Frames that change nothing cost nothing
- **WHEN** the application redraws repeatedly with nothing changing the document
- **THEN** the engine is asked for the memory figures at most once a second, and the frames in between show the figures already read

#### Scenario: A change is on the meter within a second
- **WHEN** the memory in use changes
- **THEN** the status area shows the new figure within a second of the change

#### Scenario: Another document is not reported with the last one's figures
- **WHEN** the open document is replaced
- **THEN** the next frame takes a fresh reading rather than waiting out the interval

#### Scenario: Approaching the budget is visible before it is reached
- **WHEN** memory in use approaches the configured budget
- **THEN** the indicator changes state before the budget is exhausted, rather than only at failure

#### Scenario: Listening is visible
- **WHEN** the application is listening for an agent
- **THEN** the status area says so, and says whether a client is connected

#### Scenario: A change made by an agent is attributable
- **WHEN** an agent changes the document
- **THEN** the status area records that an agent acted and when
