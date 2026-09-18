## Purpose

A single machine-readable file describing what a working session cost and the
conditions it cost it under, written to be handed to the engine's authors by
someone who cannot hand over the machine, the document, or the conversation
that produced it.

## ADDED Requirements

### Requirement: A session's cost can be exported as one file
The application SHALL offer, from the same place the diagnostics report is
offered, an export that writes the session's profile to a single file the user
chooses. The file SHALL be machine-readable and self-contained: a reader with
nothing but the file SHALL be able to state what was measured, on what, and
under what conditions, without asking a follow-up question.

The export SHALL be available whenever a document is open, including when
nothing has been measured yet. A profile with no samples is a fact about the
session — it says the operations in question did not run — and is distinct from
a profile that could not be written.

#### Scenario: A profile is written
- **WHEN** a person exports the profile after sculpting
- **THEN** a single file is written to the path they chose, and it parses as
  well-formed JSON

#### Scenario: Nothing was measured
- **WHEN** a person exports the profile from a session in which no stroke has
  been applied
- **THEN** the file is still written, and states a sample count of zero for
  each phase rather than omitting the phases

#### Scenario: The file cannot be written
- **WHEN** the chosen path cannot be written to
- **THEN** the application reports it and continues, and no partial file is
  left behind

### Requirement: The file says which build produced it, and whether its timings mean anything
An unoptimised build runs this work materially slower than the build that
ships, so a duration taken from one is a fact about the build profile rather
than about the engine.

The exported profile SHALL state the build profile it was taken from, and SHALL
carry an explicit statement of whether its durations are comparable to
durations taken elsewhere. A profile from an unoptimised build SHALL declare
that they are not. The application SHALL state the same thing to the person
exporting it, before the file is written.

#### Scenario: A debug build marks its own numbers
- **WHEN** the profile is exported from an unoptimised build
- **THEN** the file names that build profile and declares its timings not
  comparable, and the person is told so at the point of export

#### Scenario: A release build says its numbers stand
- **WHEN** the profile is exported from an optimised build
- **THEN** the file names that build profile and declares its timings
  comparable

### Requirement: The file carries the conditions the numbers were taken under
A duration without its conditions cannot be compared with any other duration,
and a comparison across unlike conditions is worse than no comparison.

The exported profile SHALL carry, for the session it describes: the
application's version; the engine's version **and the revision of the engine
build that was linked**, because two builds reporting the same version can
differ by a commit; the platform and architecture; every backend the engine
registered, which one is active, and why; the graphics adapter; and the
viewport the frames were drawn at.

#### Scenario: The engine is identified by build, not only by version
- **WHEN** a reader opens an exported profile
- **THEN** it names both the engine version and the revision of the engine that
  was linked, and where the revision is unknown it says so rather than
  reporting an empty or invented value

### Requirement: The file carries the shape of the work the numbers describe
A millisecond figure is meaningless without what it was spent on. The exported
profile SHALL describe the document the session measured: how many subtools it
holds and in which representations, and for each measured phase the size of the
work each sample covered — at least the keys re-meshed and the triangles
produced.

It SHALL carry the document's memory broken down the way the diagnostics report
breaks it down, so that a cost can be read against what was resident when it
was paid.

#### Scenario: A duration is accompanied by its workload
- **WHEN** a reader reads a per-phase duration in the file
- **THEN** the file also states how much work the samples behind it covered, in
  keys and triangles

### Requirement: The file separates the engine's time from the application's
The purpose of the file is to tell its reader which side of the boundary a cost
falls on. A total that mixes them tells the reader nothing they can act on.

The exported profile SHALL attribute each measured phase of a stroke to either
the engine or the application, and SHALL keep the engine's phases separate from
each other: the edit and refill that an applied stroke performs, and the
meshing of the bricks it dirtied, are distinct calls and SHALL be distinct
figures.

#### Scenario: A slow stroke names its slow phase
- **WHEN** a stroke is slow and the profile is exported
- **THEN** the file states, per phase, whether the time was spent inside the
  engine or inside the application, and which engine call it was spent in

### Requirement: A distribution is exported, not an average
One sample is an anecdote and a mean hides the tail a sculptor actually feels.

For every phase it reports, the exported profile SHALL carry the number of
samples, the median, the 95th percentile and the worst observed value. It SHALL
NOT report a mean in place of these.

#### Scenario: The tail survives the export
- **WHEN** most samples are fast and a few are slow
- **THEN** the file reports the worst observed value and the 95th percentile
  alongside the median, so the slow ones are visible

### Requirement: The file carries what the session already knew and could not otherwise say
The application already observes several things that bear directly on engine
performance and currently reach no reader. The exported profile SHALL carry:
every operation that held the interface thread longer than one frame, with its
worst time and its occurrence count; every operation that fell back to another
backend and which backend declined it; the per-pass GPU milliseconds where the
adapter measures them, and an explicit statement where it does not; and the
measured cost per brick of a refill on each backend the routing considered.

Where a figure was never measured this session, the file SHALL say that,
distinctly from reporting it as zero.

#### Scenario: The refill routing evidence is exported
- **WHEN** the session has refilled bricks on more than one backend
- **THEN** the file states the measured cost per brick on each, which is the
  evidence the routing decision was made on

#### Scenario: An unmeasured figure is not reported as zero
- **WHEN** the adapter offers no GPU timestamps, or a backend was never
  measured
- **THEN** the file says the figure was not measured, and does not carry a zero
  in its place

### Requirement: The export carries no more about the person than the report does
The file is written to be attached to a public issue.

The exported profile SHALL carry no absolute filesystem path, no document name,
and no content of the user's work. Where a measured item needs identifying, it
SHALL be identified by its representation and its position in the document
rather than by a name the user chose.

#### Scenario: A named subtool does not carry its name out
- **WHEN** a document whose subtools the user has named is profiled and
  exported
- **THEN** the file describes those subtools by representation and index, and
  contains none of the chosen names

### Requirement: Assembling the report costs nothing where nothing will read it
Measurement that changes what it measures is not measurement. Two costs are
distinct here and SHALL be treated as distinct: **recording** a phase, which
happens on every dab of every stroke, and **summarising** the session, which
sorts every retained window.

Recording SHALL remain negligible beside the work it measures, and SHALL NOT be
switchable — a control that buys nothing is a control that has to be explained,
remembered and tested.

Summarising SHALL happen only where something is going to read the result: the
window that displays it, the file being written, or a request that asked for it.
A report assembled with nothing to read it SHALL carry no stroke section.

An absent section and a section reporting no samples SHALL remain
distinguishable: the first says nobody asked, the second says nothing was
measured.

#### Scenario: An idle frame pays nothing
- **WHEN** frames are drawn with the diagnostics window closed and no request
  outstanding
- **THEN** no session profile is summarised, and the report carries no stroke
  section

#### Scenario: Recording stays beneath the work it measures
- **WHEN** a phase is recorded
- **THEN** the cost of recording it is negligible against the operation being
  measured, and is asserted to be so

### Requirement: An agent can read where the strokes spent their time
A party driving the session through the agent-facing interface SHALL be able to
read the same per-phase attribution without opening a panel, writing a file, or
changing anything about the session.

The reading SHALL name, for each phase, which side of the engine boundary the
time was spent on, and SHALL carry a distribution rather than an average.

Every figure read this way SHALL state that it came from a live session, so
that it cannot be mistaken for, or recorded as, a benchmark baseline.

The command that writes the profile to a file SHALL NOT be offered to an agent:
it opens a file panel on the sculptor's own screen, which an agent cannot
answer.

#### Scenario: An agent asks what its strokes cost
- **WHEN** an agent has driven strokes and reads the stroke section
- **THEN** it receives each phase with its side of the boundary, its sample
  count and its distribution

#### Scenario: A live figure is marked as one
- **WHEN** any stroke figure is read through the agent-facing interface
- **THEN** it states that it was taken from a live session
