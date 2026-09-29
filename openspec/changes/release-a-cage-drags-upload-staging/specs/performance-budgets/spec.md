## ADDED Requirements

### Requirement: A cage drag's process footprint is recorded
The benchmark SHALL report the process footprint after a 100-frame
single-point mesh cage drag, with the cage cancelled, over the footprint before
the drag, as `cage.footprint` with a budget of 1.2. Where the platform cannot
read the footprint, the figure SHALL be skipped with that reason, not dropped.

#### Scenario: A long drag returns the footprint
- **WHEN** the benchmark runs with a headless GPU on a platform that reads the footprint
- **THEN** it reports `cage.footprint` against a budget of 1.2

#### Scenario: The footprint cannot be read
- **WHEN** the platform gives no footprint
- **THEN** `cage.footprint` is skipped with the reason that the footprint cannot be read
