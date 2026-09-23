## ADDED Requirements

### Requirement: A figure records the spread it was reduced from
A benchmark figure is a summary of several samples, and a summary on its own
cannot say whether a change is a change. The harness SHALL record, beside every
figure it reduces from samples, how many samples there were and the range they
covered, and SHALL write that alongside the figures in the recorded file.

A measurement that genuinely has one observation SHALL record no spread and SHALL
be shown as having none, rather than being given a range of zero width. The
difference between "measured twelve times, all within a millisecond" and
"measured once" is the difference the reader needs.

The spread SHALL be written as a section beside the figures rather than by
changing a figure's own shape, so that a file recorded before this existed still
compares and a file recorded after it still opens in a reader that does not know
about it.

A comparison SHALL be allowed to say that a change landed inside the range the
baseline's own samples covered, and SHALL **mark** such a change rather than
excusing it. Within-run spread is the smaller half of the noise: the variance
that dominates is between runs, which one process cannot sample, so a change
inside the spread is a change that was never distinguishable — not a change that
has been ruled out.

#### Scenario: A recorded figure carries its samples
- **WHEN** a run records its figures to a file
- **THEN** each figure reduced from more than one sample is accompanied by the
  number of samples and the range they covered

#### Scenario: A single observation says so
- **WHEN** a figure is a single observation or a derived ratio
- **THEN** it records no spread, and the report shows that it has none rather
  than showing a zero range

#### Scenario: A baseline recorded without spread still compares
- **WHEN** a run is compared against a baseline recorded before spread was written
- **THEN** the comparison proceeds and reports the change, saying only that the
  baseline recorded no spread

#### Scenario: A change inside the spread is marked, not excused
- **WHEN** a figure moves but lands inside the range the baseline's own samples
  covered
- **THEN** the comparison says so beside the row, and still reports the change

### Requirement: The conditions name which build of the engine, not only which version
Two builds can both report the same engine version and differ by a commit. The
recorded conditions SHALL carry the vendored engine's revision beside its
version, so that a comparison across two recordings can state which engines it is
actually comparing.

The revision SHALL NOT be compared. A comparison across two engine builds is the
measurement an upgrade needs, and refusing it would remove the only tool for
taking it. Instead the report SHALL announce, above the table, when the two sides
were recorded against different engines, so that every percentage below is read
as that difference plus whatever else moved.

A source tree with no revision available SHALL say that it recorded none rather
than failing to record at all.

#### Scenario: The recorded file names the engine build
- **WHEN** a run records its conditions
- **THEN** the file carries the engine's version and the vendored engine's
  revision

#### Scenario: A cross-build comparison is announced rather than refused
- **WHEN** a run is compared against a baseline recorded against a different
  engine
- **THEN** the comparison proceeds, and a note above the table names both engines
