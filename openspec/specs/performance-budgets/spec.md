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

#### Scenario: A local edit in a large scene
- **WHEN** the same small edit is applied to the reference scene and to a scene ten times larger in surface area
- **THEN** the bricks re-evaluated and re-meshed are equivalent in both cases, and the measured cost does not scale with the larger scene

### Requirement: Interface responsiveness is independent of engine work
The application SHALL remain responsive to input while engine work is in progress: no engine operation SHALL block the interface thread for more than 16 ms.

#### Scenario: A long operation does not freeze the window
- **WHEN** a consolidation, bake, import or export runs
- **THEN** the window continues to redraw and respond, and progress is displayed

#### Scenario: Interface-thread blocking is detectable
- **WHEN** the application runs with the debug instrumentation enabled
- **THEN** any interface-thread block exceeding 16 ms is recorded with the operation responsible

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

