## MODIFIED Requirements

### Requirement: Regional consolidation is reachable and its signals are read correctly
The bridge SHALL expose planning and performing a regional consolidation, and callers SHALL read its result by the rules below.

#### Scenario: Planning without baking
- **WHEN** a caller asks what a regional consolidation would absorb
- **THEN** the answer is returned without modifying the document

#### Scenario: Reading local against whole
- **WHEN** a caller needs to know whether a consolidation stayed local
- **THEN** the merge's local-vs-whole answer is the value read
- **AND** the absorbed count is read only where a layer carries more than one root

#### Scenario: A pin that reintroduces a ratcheting closure
- **WHEN** an engine pin reports a widening closure for a request held fixed
- **THEN** the bridge's own coverage fails rather than the behaviour reaching a sculptor
