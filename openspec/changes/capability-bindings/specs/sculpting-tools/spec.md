## ADDED Requirements

### Requirement: Capability bindings are executable evidence
Each sculpting tool the interface presents SHALL have one binding for the
active representation in the authoritative capability table. A binding SHALL
name the artist intent, execution family, exact ClayCore entry point or
ordered recipe, distinguishing parameters, and fidelity supported by a
behavioural test. The named call and parameters SHALL be the ones dispatch
executes. A tool without a binding SHALL be absent from that representation's
default shelf and SHALL carry a `ToolNote` explaining where it applies.

The shelf, availability rule, agent catalogue, diagnostics and tests SHALL
read this table. No independent capability list SHALL decide whether a tool
applies. Layer operations SHALL remain separate from brush bindings. A tool
SHALL NOT trigger an implicit representation conversion.

#### Scenario: The declared verb is the executed verb
- **WHEN** every offered tool and representation pair is exercised on a fixture
- **THEN** the observed engine call and parameters match its binding, and the named symbol exists in the pinned engine

#### Scenario: A claimed distinction is visible
- **WHEN** two tools are offered on the same representation with distinct intents
- **THEN** a behavioural fixture distinguishes their results by a documented measurement, or one binding is removed with an explanatory note

#### Scenario: A recipe tells the truth
- **WHEN** voxel Crease is offered as a narrow-erosion recipe
- **THEN** its binding lists the operations executed and does not claim a dedicated Crease engine symbol

#### Scenario: An unavailable tool stays absent
- **WHEN** a tool has no binding on the active representation
- **THEN** it is absent from the default shelf and cannot silently convert the layer to use another representation's verb
