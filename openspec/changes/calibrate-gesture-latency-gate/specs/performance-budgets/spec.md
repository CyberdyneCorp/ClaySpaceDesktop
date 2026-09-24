## ADDED Requirements

### Requirement: The release gesture gate calibrates to its runner
The release gesture regression test SHALL time a fixed full rebuild of the same reference scene in the same process as its incremental gesture. The worst live segment SHALL take no more than one quarter of that rebuild, and pointer-up SHALL take no more than one tenth. The test SHALL report raw timings and its reference timing on every run. Debug builds SHALL report timings without asserting a release performance ratio.

#### Scenario: The runner is slower without a code regression
- **WHEN** the gesture and its full-rebuild reference both scale with a slower runner
- **THEN** the release gate gives the same verdict for the same relative work

#### Scenario: A live segment loses its incremental path
- **WHEN** the worst segment approaches the cost of rebuilding the whole reference scene
- **THEN** the release gate fails and reports the segment and rebuild timings

#### Scenario: Pointer-up regains a whole-scene pass
- **WHEN** the end of a gesture costs more than one tenth of the reference rebuild
- **THEN** the release gate fails and reports the pointer-up and rebuild timings

### Requirement: The release dab gate calibrates to its runner
The release dab regression test SHALL time a fixed full rebuild of its own 24-dab reference scene in the same process. Median dab latency SHALL be no more than one fifth of the rebuild, and the 95th percentile SHALL be no more than one third. Raw timings SHALL continue to be reported so the separately specified 50/100 ms product targets can be evaluated on a named reference machine.

#### Scenario: A shared runner is slower without a dab-path regression
- **WHEN** dab timings and the fixed full rebuild scale together on a slower runner
- **THEN** the release gate gives the same verdict for the same relative work

#### Scenario: Dabs begin remeshing most of the form
- **WHEN** median or 95th-percentile dab cost approaches the full rebuild
- **THEN** the release gate fails and reports the dab and rebuild timings
