## MODIFIED Requirements

### Requirement: Memory stays within the configured budget
The application SHALL configure the engine's brick cache memory budget, SHALL keep the brick cache within it during normal sculpting on the reference scene, and SHALL handle the engine's budget-exceeded result as a reported condition rather than a failure.

The budget bounds the brick cache and nothing else. The figure in use also
counts the document, its surfaces and what the application holds to draw, none
of which the budget limits, so that figure MAY exceed the budget without the
budget having been exceeded.

#### Scenario: Sustained sculpting stays within budget
- **WHEN** a sustained sculpting session runs on the reference scene
- **THEN** the brick cache stays within the configured budget

#### Scenario: A session does not leak
- **WHEN** a document is opened, sculpted, and closed repeatedly
- **THEN** memory returns to its baseline after each close, within a stated tolerance
