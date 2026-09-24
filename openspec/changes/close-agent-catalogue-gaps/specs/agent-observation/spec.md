## ADDED Requirements

### Requirement: Agent-visible outstanding work includes geometry jobs
Retopology, UV atlas, conform and texture bake jobs SHALL appear in the
session's outstanding work while active, whether started by a normal tool call
or by `measure`. `state.jobs` SHALL list their names and available progress,
and `wait` SHALL NOT report quiet until the application has collected the job
result.

#### Scenario: Measurement starts a retopology job
- **WHEN** a measured retopology action starts a worker job
- **THEN** `state.jobs` names the job and `wait` reports it as outstanding
- **AND** after the application collects its result, neither reports it as
  outstanding
