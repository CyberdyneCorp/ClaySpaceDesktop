## ADDED Requirements

### Requirement: Performance is gated on every platform the application ships on
CI SHALL run the benchmark on macOS and on Linux and SHALL compare each run
against a baseline recorded on the same runner image, committed to the
repository, one file per platform.

The comparison SHALL fail the job when a figure regresses beyond its tolerance
multiplied by a stated scale, when a figure the baseline measured is missing or
newly skipped, or when the run refuses to compare against the baseline. The
scale SHALL be derived from measured run-to-run variance on that runner, SHALL
be stated in the repository documentation with the measurement behind it, and
SHALL NOT be below 1: the tolerances themselves are what a quiet reference
machine was measured to allow.

#### Scenario: A regression beyond the stated threshold fails CI
- **WHEN** a change makes a figure worse than its baseline by more than its
  tolerance times the CI scale, on either platform
- **THEN** that platform's Performance job fails and names the figure

#### Scenario: Noise between two runs of an unchanged tree does not fail CI
- **WHEN** the benchmark runs twice on the same runner image on a tree that did
  not change the measured code
- **THEN** no figure differs by more than the stated threshold

### Requirement: A baseline names the machine that recorded it
The recorded conditions SHALL name the recording machine — processor, logical
cores, memory, operating system, and the CI runner image where there is one —
and a comparison against a baseline recorded on a different processor, core
count or memory SHALL say so above its table. A baseline recorded before this
was kept SHALL be announced as naming no machine rather than refused.

#### Scenario: The file says which machine
- **WHEN** a run records a baseline
- **THEN** its conditions carry the machine's specification beside the figures

#### Scenario: A workstation compared against a runner's baseline
- **WHEN** a run on one machine is compared against a baseline recorded on
  another
- **THEN** the comparison proceeds and a note names both machines
