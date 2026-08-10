## ADDED Requirements

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
