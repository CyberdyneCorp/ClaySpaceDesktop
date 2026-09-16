## ADDED Requirements

### Requirement: A degraded field layer collapses the region it was worked in
A field layer whose deformer chain has degraded past the configured floor SHALL, after a gesture is committed, collapse the region that gesture touched into a single baked volume, and SHALL leave the surface it just drew unchanged.

#### Scenario: A worked patch stops accumulating
- **WHEN** a sculptor works one patch of a field layer with repeated Move gestures
- **THEN** the layer's deformer chain returns to zero rather than growing one link per dab per mirror image
- **AND** the collapse happens after the gesture is committed, so no surface a sculptor drew is altered by it

#### Scenario: A layer that never degrades is never collapsed
- **WHEN** a field layer's chain stays above the floor for a whole session
- **THEN** no bake is performed and the session costs nothing for this capability

#### Scenario: The decision itself is free
- **WHEN** the end of every field gesture evaluates whether a collapse is due
- **THEN** that evaluation performs no bake and does not change the document

### Requirement: The collapse is reported as local or whole-layer
The host SHALL determine whether a collapse was local or whole-layer from the merge's own local-vs-whole answer, and SHALL NOT infer it by comparing the absorbed root count against the layer's root count.

#### Scenario: A single-root layer
- **WHEN** a layer has exactly one root, which is every document until a sculptor adds a subtool
- **THEN** the absorbed count equals the root count for a local collapse and for a whole-layer one alike, so it cannot distinguish them
- **AND** the local-vs-whole answer is what the host reads

#### Scenario: A collapse that took the whole layer
- **WHEN** the closure reaches every visible root, as it does on the first collapse of a fresh form
- **THEN** the host records that the collapse was whole-layer rather than local
- **AND** treats it as the honest fallback rather than as a failure

### Requirement: A ratcheting closure is detected rather than absorbed
The host SHALL watch the closure the engine reports against a request held fixed, and SHALL stop collapsing that layer if the closure grows across successive collapses.

#### Scenario: The closure stays put
- **WHEN** successive collapses are requested for the same region
- **THEN** the reported closure does not widen, and collapsing continues

#### Scenario: The closure widens
- **WHEN** the reported closure widens across successive collapses for an unchanged request
- **THEN** the host stops collapsing that layer and says why, rather than paying a cost that compounds

### Requirement: A collapse a sculptor waits for is a collapse a sculptor is told about
A collapse that occupies the interface SHALL be visible as work in progress, and its cost SHALL be attributable in the session's own diagnostics.

#### Scenario: The pause is legible
- **WHEN** a collapse runs at the end of a gesture
- **THEN** the interface reports that the layer is being compacted rather than appearing to have stalled

#### Scenario: The cost is attributable
- **WHEN** a session is examined afterwards
- **THEN** the time spent collapsing is reported separately from the time spent sculpting
