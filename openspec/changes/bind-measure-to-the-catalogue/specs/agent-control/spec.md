## ADDED Requirements

### Requirement: The catalogue bounds every route and names what it withholds
Every call that builds a command from a group and an action — a group tool and
`measure` alike — SHALL be refused, changing nothing, when the group is not
declared in the catalogue's groups or the action has no catalogue row. The
dispatch SHALL NOT be reachable except through what `describe` offers.

`describe` SHALL list, with its reason, every command the application places as
deliberately not offered to agents, so that a caller can tell a command that
exists but is withheld from one that does not exist.

Each action's summary SHALL describe what the action does, including where it
does nothing, and SHALL be checked by a test against the command or binding it
describes.

#### Scenario: Measure cannot reach an undeclared group
- **WHEN** a client calls `measure` with a group the catalogue does not declare
- **THEN** it is refused as an unknown group and nothing is applied

#### Scenario: Every withheld command is listed
- **WHEN** the commands `describe` lists as not offered are compared with every
  command the application places as not offered
- **THEN** the two sets are equal, and each entry carries its reason

#### Scenario: The manipulator's flag is named for what it does
- **WHEN** a client calls `transform` `drag` with `snap: true`
- **THEN** the drag snaps a rotation to whole increments
- **AND** the same call with `invert` is refused as an undeclared argument
