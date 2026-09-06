## ADDED Requirements

### Requirement: The pinned ABI and the container minor are constants a test holds
The safe wrapper SHALL name the engine ABI it was written against, and the
`.clayspace` container minor this build writes, as constants that a test checks
against the linked engine rather than against themselves.

Neither SHALL be derived from the linked engine. A constant read back from the
thing it is meant to check makes the check assert that a number equals itself,
and the whole value of these two is that moving the submodule without moving
them **fails**.

The container minor SHALL carry, beside it, why this build writes that minor
rather than an older one — including where the upgrade notes advise writing
older and that advice cannot be taken through this ABI, so that a reader meets
the reasoning rather than reconstructing it.

#### Scenario: The pin moves and the constants do not
- **WHEN** the vendored engine is pointed at a release whose ABI minor or
  container minor differs from the constants
- **THEN** the wrapper's own tests fail, naming both numbers and saying which
  one moved

#### Scenario: A file says what the build claims
- **WHEN** this build writes a `.clayspace`
- **THEN** the minor in the file's own header is the minor the constant claims

### Requirement: A document written now is refused by an older build, not misread
The container format SHALL fail in the direction that cannot corrupt work: a
build predating the pinned engine SHALL refuse a document this build writes
rather than read it as something else.

The application SHALL record which minor it writes where a person can find it,
because the consequence — a document that will not open elsewhere — is one a
sculptor meets and not one a maintainer does.

#### Scenario: An older build meets a newer document
- **WHEN** a build older than the pinned engine opens a document this build
  wrote
- **THEN** it refuses the file rather than reading the records out of step
