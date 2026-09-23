# performance-budgets Specification

## Purpose
The numbers the application is held to — brush latency, frame rate, startup,
memory, and the rule that edit cost follows the region edited — together with
how they are measured in CI rather than asserted, and when work that is not
needed for correctness is allowed to run.

## Requirements

### Requirement: A reference scene defines what the budgets are measured against
The project SHALL define a reference document and a reference machine configuration for each supported platform, and every performance budget SHALL be stated and measured against them. Budgets SHALL NOT be asserted against an unspecified scene.

#### Scenario: Budgets name their conditions
- **WHEN** a performance budget is reported
- **THEN** it names the reference document, the platform, the active backend and the viewport resolution it was measured at

### Requirement: Brush feedback appears within a stated latency
From the completion of a brush dab's input event to that dab being visible in the viewport, the application SHALL stay within 50 ms at the median and 100 ms at the 95th percentile on the reference scene and machine, with a GPU backend active. On the CPU backend the budget SHALL be stated separately rather than treated as a failure.

#### Scenario: Median dab latency holds
- **WHEN** a continuous stroke is applied to the reference scene with a GPU backend active
- **THEN** the median input-to-visible latency is at most 50 ms and the 95th percentile is at most 100 ms

#### Scenario: A regression fails the gate
- **WHEN** a change raises measured dab latency beyond its budget on the reference scene
- **THEN** the performance gate fails and reports the before and after figures

### Requirement: The viewport sustains an interactive frame rate
The viewport SHALL sustain at least 60 frames per second while orbiting the reference scene with no edit in progress, and SHALL NOT drop below 30 frames per second during a continuous stroke.

#### Scenario: Camera movement stays smooth
- **WHEN** the user orbits the reference scene continuously
- **THEN** the frame rate remains at or above 60 frames per second

#### Scenario: Sculpting does not stall the view
- **WHEN** a continuous stroke is applied while the camera is moving
- **THEN** the frame rate remains at or above 30 frames per second

### Requirement: Edit cost is proportional to the region edited
The work performed for an edit SHALL be bounded by the region the edit's influence bound reaches, and SHALL NOT grow with the size of the rest of the document.

Taking an edit back SHALL be bounded the same way as making it. Retiring a
placed sweep SHALL re-evaluate the region the sweep occupied — the engine's own
bound for that item, asked for while the document still holds it — and SHALL
NOT re-evaluate the subtool the sweep was laid on, nor the box that subtool
occupied before the removal.

An operation that borrows a visibility pattern for its own length — sampling
one subtool alone, writing a file under a solo — SHALL re-evaluate nothing for
the flags it writes, because it restores the pattern before anything reads the
surface and the field is therefore the same field on both sides of it. Where a
restore does not complete, the layers left off the sculptor's pattern SHALL be
re-evaluated after all.

#### Scenario: A local edit in a large scene
- **WHEN** the same small edit is applied to the reference scene and to a scene ten times larger in surface area
- **THEN** the bricks re-evaluated and re-meshed are equivalent in both cases, and the measured cost does not scale with the larger scene

#### Scenario: Cancelling a curve costs the tube
- **WHEN** a curve is laid on a worked subtool and then cancelled
- **THEN** the bricks re-evaluated are the tube's own, the subtool's surface is
  left as it was, and no brick is left holding a sweep that is gone

#### Scenario: A boolean does not re-evaluate the subtools it never named
- **WHEN** a boolean runs over two subtools in a document holding others
- **THEN** no brick belonging to a subtool the boolean did not name is
  re-evaluated, and every subtool's visibility is what it was before

### Requirement: Interface responsiveness is independent of engine work
The application SHALL remain responsive to input while engine work is in progress: no engine operation SHALL block the interface thread for more than 16 ms.

Re-evaluating the surface cache SHALL be bounded by a budget the host sets and
SHALL be continued across frames rather than run to completion on the caller's
thread. The cache SHALL keep whatever a bounded drain did not take, so the work
is stopped and not dropped; the document SHALL report that a refill is
outstanding, and the host SHALL keep asking for frames until it is not. A
document with no budget set SHALL drain in full, because a caller with nothing
waiting on it needs the exact answer before it returns.

The bound SHALL apply to the batches as well as to the gaps between them: a
budget honoured only between batches is that budget plus a whole batch, which
on a slow backend is most of a frame.

Whether a region was re-evaluated in one drain or in many, the surface that
results SHALL be the same surface.

#### Scenario: A long operation does not freeze the window
- **WHEN** a consolidation, bake, import or export runs
- **THEN** the window continues to redraw and respond, and progress is displayed

#### Scenario: Interface-thread blocking is detectable
- **WHEN** the application runs with the debug instrumentation enabled
- **THEN** any interface-thread block exceeding 16 ms is recorded with the operation responsible

#### Scenario: A refill larger than one budget is continued
- **WHEN** a command dirties more of the field than one budget can re-evaluate
- **THEN** the command returns within the budget, the document reports the
  refill as outstanding, and the remaining bricks are re-evaluated over the
  following frames

#### Scenario: A pumped refill reaches the surface a whole drain reaches
- **WHEN** the same edit is drained in full by one caller and a budget at a
  time by another
- **THEN** both documents hold the same surface

#### Scenario: Showing or hiding a worked subtool returns within a frame
- **WHEN** the sculptor toggles the eye on a field subtool of any size
- **THEN** the write returns within the refill budget, and the surface the
  toggle changes is completed over the frames that follow

### Requirement: Startup reaches an interactive state within a stated time
The application SHALL present an interactive window within 2 seconds of launch on the reference machine, including backend discovery. Backend discovery SHALL NOT delay the window beyond that budget.

#### Scenario: Slow backend enumeration does not delay the window
- **WHEN** backend discovery is slow because a GPU runtime is enumerating devices
- **THEN** the window still appears within the budget and reports its backend when discovery completes

### Requirement: Memory stays within the configured budget
The application SHALL configure the engine's brick cache memory budget, SHALL keep total memory in use within it during normal sculpting on the reference scene, and SHALL handle the engine's budget-exceeded result as a reported condition rather than a failure.

#### Scenario: Sustained sculpting stays within budget
- **WHEN** a sustained sculpting session runs on the reference scene
- **THEN** memory in use stays within the configured budget

#### Scenario: A session does not leak
- **WHEN** a document is opened, sculpted, and closed repeatedly
- **THEN** memory returns to its baseline after each close, within a stated tolerance

### Requirement: Performance is measured in CI, not asserted
The project SHALL include a repeatable benchmark exercising dab latency, frame time, edit locality, startup and memory against the reference scene, runnable locally and in CI, reporting figures that can be compared across revisions.

#### Scenario: Benchmarks are comparable across revisions
- **WHEN** the benchmark runs on two revisions on the same machine
- **THEN** it produces figures for the same measurements under the same conditions, suitable for direct comparison

### Requirement: GPU time is measured per pass, not inferred
The application SHALL measure GPU execution time per named render pass using
timestamp queries where the adapter supports them, and SHALL render normally,
reporting that timing is unavailable, where it does not. Timestamp support
SHALL NOT be a device requirement.

#### Scenario: An adapter without timestamps still renders
- **WHEN** the application runs on an adapter that does not support timestamp
  queries
- **THEN** the viewport renders as it otherwise would and the diagnostics view
  reports that GPU timing is unavailable

#### Scenario: Per-pass time is attributable
- **WHEN** GPU timing is available and a frame is drawn
- **THEN** the scene pass, the depth reduction, the occlusion pass, the
  composite, the overlays and the interface are each reported separately

### Requirement: Rendering has recorded benchmarks
The project SHALL carry deterministic offscreen render benchmarks over a stated
set of scenes and viewport sizes, reporting GPU frame time, per-pass time, draw
call count and bytes uploaded, and SHALL record a baseline so that a change to
the renderer can be compared against it rather than described.

#### Scenario: A rendering change is measured
- **WHEN** a change to the rendering path is proposed
- **THEN** the render benchmarks are run against the recorded baseline and the
  per-pass difference at each viewport size is reported

#### Scenario: Occlusion at high resolution costs less than it did
- **WHEN** the occlusion path is measured at 2560×1440 and above against the
  recorded full-resolution baseline
- **THEN** its GPU time is materially lower

### Requirement: Work that is not required for correctness happens between interactions
The application SHALL keep the work that makes the next interaction cheaper —
rebuilding a spatial index whose partition has decayed under editing being the
one it can currently produce — separate from the interaction that made it
necessary, and SHALL perform it only at a moment when no gesture is open.

That separation SHALL be a mechanism rather than a convention: the queue SHALL
be unreachable for draining while a gesture is open, and SHALL become reachable
again when the gesture ends *by any route* — committed, cancelled, abandoned, or
taken down with the document. A gesture that ends without saying so SHALL NOT
leave the application unable to do maintenance for the rest of the session.

Nothing queued this way is correctness. Work declined, deferred indefinitely or
never performed SHALL leave the document exactly as it was.

#### Scenario: Nothing is serviced with a pointer down
- **WHEN** work is queued during an open gesture and a drain is asked for
- **THEN** nothing is serviced and the work is still queued

#### Scenario: The moment a gesture ends is the moment it is serviced
- **WHEN** the gesture ends
- **THEN** the queued work is serviced without a further request

#### Scenario: A gesture that never ended cleanly still releases the gate
- **WHEN** a press arrives over a gesture that was never closed, and that
  gesture then ends
- **THEN** the queue is drainable rather than held shut

### Requirement: The between-strokes drain is budgeted, and states what its budget was chosen against
The drain SHALL run against a stated time budget rather than to completion. The
budget SHALL be named where it is defined, together with what it was chosen
against, and SHALL be no more than half of the interface-thread bound the
specification allows a single engine operation.

An item SHALL be started only where what remains of the budget covers the
estimate it carries. Work that does not fit SHALL be left queued rather than
performed or dropped, and SHALL remain visible with a count of how often it has
been asked for, so that work the application is starving can be seen rather than
inferred.

An estimate SHALL be measured on the machine that will pay it rather than
assumed: the first of a kind SHALL be filed with no estimate and timed, and
every request of that kind afterwards SHALL carry what was measured.

#### Scenario: A moment that cannot afford everything leaves the rest
- **WHEN** the queue holds more work than the budget covers
- **THEN** what fits is serviced, what does not is still queued, and the drain
  stops rather than overrunning

#### Scenario: Declining is not dropping
- **WHEN** a later moment can afford the work that was left
- **THEN** the same work is serviced and nothing was lost

#### Scenario: An estimate is what this machine measured
- **WHEN** work of a kind has been performed once
- **THEN** the next request of that kind carries the measured figure rather
  than a guess

### Requirement: A gesture holds a memory pin
While a gesture is open the application SHALL hold a memory pin, so that a trim
arriving mid-drag reports what it would have released and releases nothing. The
engine prices a trim's cost to the interaction after it — between 0.62 and 2.04
times at the gentlest pressure and between 13 and 182 times at the hardest,
growing with the model — and a drag is the one moment where that cost is certain
to be paid by the sculptor.

The pin SHALL be given back on every way a gesture ends, and SHALL be taken and
given back at exactly the moments the maintenance gate is, so that the two
cannot come apart.

#### Scenario: The pin follows the pointer
- **WHEN** a gesture opens
- **THEN** the pin is held, and it is given back when the gesture ends however
  it ends

#### Scenario: A cage is a gesture too
- **WHEN** a deformation cage is dragged and then applied, or abandoned
- **THEN** the pin was held for the drag and is given back either way

### Requirement: An idle application does no work proportional to the document
With a document open, no gesture in progress, no command running and no
animation, the application SHALL NOT perform per-frame work whose cost grows
with the size of the document. An application nobody is touching SHALL settle
to a small, flat cost that a worked sculpture does not raise.

A figure that is only displayed — a meter, a counter, a status line — SHALL be
refreshed on a stated interval rather than once a frame where obtaining it
costs more than reading a field. The interval SHALL be named where it is
defined, and SHALL be short enough that a person cannot tell the figure from an
exact one.

This is a budget in its own right and not only a matter of power: work paid on
every idle frame is the noise floor of every other measurement taken of this
application, and a profile whose largest entry is a status bar cannot be used
to find anything else.

#### Scenario: Idling with a worked document is cheap
- **WHEN** a document with a worked layer is left open with no input for a
  minute
- **THEN** the application uses a small fraction of one core, and the figure
  does not rise with the size of the sculpture

#### Scenario: A displayed figure is refreshed on an interval
- **WHEN** a figure shown in the interface costs a walk of an engine structure
  to obtain
- **THEN** it is obtained at most once per stated interval, and the frames in
  between show the figure already obtained

#### Scenario: The interval is short enough to be honest
- **WHEN** the value behind such a figure changes
- **THEN** what the interface shows catches up within the stated interval

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
