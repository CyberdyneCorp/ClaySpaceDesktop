## MODIFIED Requirements

### Requirement: Specifications are validated in CI
Continuous integration SHALL validate the OpenSpec artifacts strictly on every pull request and push, and SHALL fail on a validation error.

It SHALL also check the **living specification as a whole**, which strict
validation does not: that reads one spec or one change at a time, so a rule
written down in two capabilities is two valid documents and no error. The whole
check SHALL fail where a requirement heading stands in more than one capability
— whether the two texts differ, because then neither says what the application
does, or agree, because two copies of one rule drift — and where a capability
still carries the placeholder Purpose the archive writes, which names a change
rather than saying what the capability covers.

A capability may be declared as one two specs share, with the reason recorded
beside the declaration. The check SHALL read that list rather than a comment, so
an intended overlap is a decision somebody wrote down.

#### Scenario: An invalid spec fails the build
- **WHEN** a specification file is malformed or a requirement lacks a scenario
- **THEN** the specification validation job fails and names the file and the problem

#### Scenario: One rule in two capabilities fails the build
- **WHEN** the same requirement heading stands in two living specs and the two
  texts differ
- **THEN** the job fails, naming both capabilities and the requirement, rather
  than passing because each file is well formed on its own

#### Scenario: A capability that never said what it is for
- **WHEN** a change is archived into a spec the archive had to create, and the
  placeholder Purpose is left as written
- **THEN** the job fails rather than leaving a capability described by the name
  of a change
