## ADDED Requirements

### Requirement: The startup banner states the agent door once
The report printed when the application starts SHALL be printed after the
agent door has been opened or has failed to open, and SHALL describe the door as
it then stands, in one line. It SHALL NOT say the build has no door while
another line gives the door's address.

#### Scenario: The banner agrees with itself
- **WHEN** the application starts and its door opens
- **THEN** the banner's agent line gives the address it is listening on, and no
  other line contradicts it
