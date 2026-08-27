## ADDED Requirements

### Requirement: A stated reason is not on its own an excuse
A figure the baseline recorded and this run did not measure SHALL fail the gate
unless the reason it was skipped is the machine's inability, or the baseline
recorded the same reason.

`skip.rs` has always drawn the distinction — "a measurement that could not run
on this machine, which is fine, or a measurement that quietly stopped running,
which is the thing a performance gate exists to catch" — and nothing acted on
it: any reason at all moved a figure into the accounted column. An engine that
started refusing every edit recorded `EditRefused` against every brush group,
dropped every brush figure, and passed.

Machine inability SHALL be excused unconditionally rather than by agreement,
because it is true whatever the code does and a developer without a GPU who
sees red learns to ignore red.

#### Scenario: An engine that started refusing every edit
- **WHEN** a figure the baseline measured is absent, skipped for a reason the
  baseline never recorded
- **THEN** the gate fails and names the figure and both reasons

#### Scenario: A machine that never could
- **WHEN** a figure is absent because there is no adapter or no backend
- **THEN** the gate does not fail, whether or not the baseline said so

#### Scenario: A reason that changed
- **WHEN** a figure is absent for a different reason than the baseline recorded
- **THEN** the gate fails, because the story changing is the signal

### Requirement: A gate that declines to compare is not a gate that passed
The benchmark SHALL report plainly when it cannot compare against the baseline
it was given, and a run that declines to compare SHALL NOT be presented as a
run that found no regression.

The performance job compared against a baseline recorded before the reference
suite existed, so the comparison was refused, `compare` returned "nothing
failed", and the job went green for every commit for months. A gate in that
state says exactly what a working gate says.

#### Scenario: The baseline cannot be compared against
- **WHEN** a run is given a baseline it refuses to compare with
- **THEN** it states the refusal and the reason, rather than reporting success

### Requirement: A crash is told apart from a verdict
A benchmark run that dies on a signal SHALL be distinguished from one that
reached a verdict, and the exit status SHALL survive whatever the CI step pipes
it through.

The step piped into `tee` under `bash -e` without `pipefail`, so every non-zero
status was discarded — including a `SIGSEGV` in the warm-up that had been
happening unnoticed. A pipeline reporting its last command's status is how a
crash becomes a pass.

Where a known crash is tolerated so that it does not fail unrelated work, the
tolerance SHALL name the issue tracking it and SHALL say when to remove itself.
A quarantine with no expiry is the swallowed exit code with better manners.

#### Scenario: The benchmark reaches a verdict
- **WHEN** the run exits non-zero having decided something — a regression, or a
  refusal to record
- **THEN** the job fails

#### Scenario: The benchmark dies on a signal
- **WHEN** the run is killed by a signal before reaching a verdict
- **THEN** the job reports the crash distinctly from a regression, and names
  where it is tracked
