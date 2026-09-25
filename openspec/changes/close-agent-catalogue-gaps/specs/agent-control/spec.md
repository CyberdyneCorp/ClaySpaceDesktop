## ADDED Requirements

### Requirement: Measurement uses the published command vocabulary
The `measure` tool SHALL accept only groups published by the agent catalogue.
It SHALL refuse an unknown group before dispatch and name the published groups
in the refusal. Measuring an offered action SHALL use its normal command path
and argument contract.

#### Scenario: An undeclared measurement group is refused
- **WHEN** a client measures an action under a group absent from `GROUPS`
- **THEN** the call is refused with the published group names and no command
  is applied

### Requirement: Deliberately unoffered commands are explained
`describe.not_offered` SHALL name each deliberately unoffered command and its
reason, including bake execution and profile export, so a client can tell a
real but unavailable operation from one that does not exist.

#### Scenario: A client asks why baking cannot be started
- **WHEN** a client asks `describe` for all groups
- **THEN** `not_offered` explains that starting a bake needs the person's
  destination file panel
