## ADDED Requirements

### Requirement: A timed series starts from a settled device
Before the first sample of a measurement is timed, the harness SHALL hand the
graphics device every write made so far, wait for the device to finish, and
account the staging as released, so that no sample pays for uploads or buffer
releases an earlier measurement left pending. The cost an application pays per
frame is paid before the clock starts, not inside a sample.

#### Scenario: What an earlier series left pending is not paid inside the next
- **WHEN** a field series has written its uploads without a frame submitting them
- **AND** the next series' screen is primed
- **THEN** the device memory figure counts no staging before that series' first sample is timed

### Requirement: A figure's samples are recorded in the order taken
The harness SHALL write, beside the spread, every sample of each timed
measurement in the order it was taken, under the measurement's name, as an
additive section of the recorded file. A file recorded before the section
existed SHALL still read, with no samples rather than an error.

A sample more than three times the next largest in its series and at least ten
milliseconds above it SHALL be announced in the run's output as it is taken,
naming its position in the series, so that a mean moved by one sample is
identified by the run that took it.

#### Scenario: A recorded run carries its samples
- **WHEN** a run records its figures to a file
- **THEN** each timed measurement's samples are in the file, in the order taken, and read back in that order

#### Scenario: A file without samples still reads
- **WHEN** a baseline recorded before the samples were kept is read
- **THEN** it reads with no samples and its figures and spread intact

#### Scenario: One sample stands apart
- **WHEN** a series of thirteen reads 1.70, 50.99, 51.10, 51.28, 51.05, 272.05, 50.80 and six samples under 2 ms
- **THEN** the run announces sample 6 of 13 at 272.05 ms against a next largest of 51.28
