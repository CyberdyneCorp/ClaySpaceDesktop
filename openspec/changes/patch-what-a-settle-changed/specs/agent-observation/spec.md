## ADDED Requirements

### Requirement: Repeated captures reuse their render target
The application SHALL keep the offscreen target a capture draws into and reuse
it for the next capture of the same size and format, replacing it only when
either changes, so that at most one capture target is held.

#### Scenario: Capture loop
- **WHEN** an agent captures repeatedly at one size
- **THEN** no new render target is allocated after the first capture

#### Scenario: Size changes
- **WHEN** a capture asks for a different size or format
- **THEN** the held target is released and replaced by one of the new size
