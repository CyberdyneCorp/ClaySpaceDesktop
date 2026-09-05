## ADDED Requirements

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
