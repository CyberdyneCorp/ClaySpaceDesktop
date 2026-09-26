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

### Requirement: Every tool on the field's shelf leaves its own mark
Each pair of stamping tools the SDF shelf offers SHALL leave marks that differ
by at least a hundredth of a unit at identical brush settings, with Acumular on
and with it off, measured across one stroke over the starting form. The
identities are:

- **Inflar** is relief with a wider region and a fifth of Padrão's lift, so its
  mark is lower and broader than Padrão's whatever Acumular says.
- **Camada** is a course of bounded height: always clamped, and shallower than
  Padrão even when Padrão is clamped too.
- **Argila** is relief with buildup and a denser stroke, so a second pass adds.

A tool that cannot be made to differ SHALL be absent from the field's shelf
with a `ToolNote` naming the tool that does the job there. **Polir** is absent
because the field has one flatten, Planar's; **Relaxar** is absent because a
field has no vertices to redistribute, and its one smooth is Suavizar's.

#### Scenario: Inflar is broader and lower than Padrão
- **WHEN** the same stroke is made on a field with Padrão and with Inflar
- **THEN** Inflar's mark rises less under the stroke and reaches further to its
  side, with Acumular on and with it off

#### Scenario: Camada differs from a clamped Padrão
- **WHEN** Acumular is off and the same stroke is made with Padrão and Camada
- **THEN** Camada's mark is the shallower, and turning Acumular on changes
  nothing about Camada

#### Scenario: An absent duplicate says where its job is done
- **WHEN** Polir or Relaxar is asked for on a field
- **THEN** it is refused with its note, and a layer switch that lands on a field
  with either in hand substitutes Planar or Suavizar

### Requirement: A field's Planar is previewed and cuts in both directions
Planar on a field SHALL be shown on the surface while the stroke is made,
without adding any entry to the history until the pointer is released, and the
released stroke SHALL land where the last preview showed it and where the same
stroke held whole lands.

Planar SHALL flatten toward one plane for the whole gesture, in dabs along the
path whose effect tapers out inside the region it samples. Held, the invert key
SHALL sink that plane into the form and cut toward it; it SHALL NOT fill, and
the cut SHALL leave no wall at the edge of its region.

#### Scenario: Planar shows itself mid-stroke
- **WHEN** a Planar stroke on an editable field is sent in segments
- **THEN** the drawn surface is cut before the release, the history is as deep
  as before the gesture, and the document is unchanged

#### Scenario: An abandoned Planar leaves nothing
- **WHEN** a previewed Planar gesture is cancelled
- **THEN** the history is as deep as before it and the drawn surface is back
  where it was

#### Scenario: Inverted Planar lowers the surface without a ledge
- **WHEN** Planar is stroked on a field with the invert key held
- **THEN** the surface under the stroke falls further than the upright stroke
  cuts it, rises nowhere across the stroke, and changes no steeper than one in
  one between neighbouring probes a cell apart
