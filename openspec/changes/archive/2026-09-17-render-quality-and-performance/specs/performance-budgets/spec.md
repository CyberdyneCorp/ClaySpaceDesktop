## ADDED Requirements

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
