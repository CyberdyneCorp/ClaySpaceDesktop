# agent-measurement Specification

## Purpose
What an agent driving the application is told about work that has not finished
yet — geometry, GPU uploads, pending mask rendering — measured without the act
of asking creating work of its own.
## Requirements
### Requirement: Measurement executes only required geometry work

The live measurement tool SHALL include command dispatch and required geometry synchronization without forcing a full rebuild solely for measurement. An owed deferred settle SHALL be completed once when allowed, and SHALL NOT be discarded during a live gesture.

#### Scenario: Idle selection
- **WHEN** measurement selects an already selected tool on a synchronized document
- **THEN** it performs no geometry upload or forced rebuild

#### Scenario: Actual stroke
- **WHEN** a measured stroke changes the field
- **THEN** required geometry reaches the renderer and the returned measurement includes that work
- **AND** completed deferred work is not repeated by the following frame

### Requirement: Wait reports pending work without creating work

Wait SHALL perform no geometry upload when the session is already quiet. It SHALL report deferred geometry work that remains blocked and SHALL stop a synchronous pass that cannot make progress.

#### Scenario: Idle wait
- **WHEN** no incremental remesh or deferred settle is pending
- **THEN** wait returns quiet with zero uploaded bytes

#### Scenario: Deferred settle under a live gesture
- **WHEN** a live gesture prevents an owed settle from running
- **THEN** wait reports the outstanding settle without discarding it or spinning until the budget expires

### Requirement: Diagnostic replies report GPU upload work

Measured and Settled replies SHALL include uploaded_bytes, the bytes written through the application's GPU upload counter during that synchronous operation.

#### Scenario: Work and timing are distinguishable
- **WHEN** an idle operation and an actual geometry change are measured
- **THEN** their upload counts distinguish no work from a geometry upload independently of elapsed time

### Requirement: Measurement includes pending mask rendering
A measured SDF mask operation SHALL synchronize changed mask attributes before returning, without forcing a geometry rebuild solely to refresh the mask.

#### Scenario: Painted mask
- **WHEN** a measured mask stroke changes frozen weights on the visible surface
- **THEN** its upload count includes the mask attribute refresh
- **AND** a subsequent idle wait does not repeat that refresh

