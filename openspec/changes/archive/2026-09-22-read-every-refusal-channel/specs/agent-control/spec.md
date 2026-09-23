## ADDED Requirements

### Requirement: A refused deformation or scene-composition command is an error
A cage, a curve, a boolean, a ZSphere rig, a cut or a reference command the
application did not carry out SHALL be answered to the client as an error
carrying the reason, and SHALL NOT be answered with an applied result.

The reason SHALL be the sentence the interface would have shown for the same
refusal. A refusal recorded on the panel's own channel and read by nobody SHALL
be treated as a refusal that was lost.

#### Scenario: A boolean the engine will not run is an error
- **WHEN** a client asks for a boolean whose operands the engine refuses —
  no pair chosen, a hierarchy offered as an operand, or a result priced past
  the memory budget
- **THEN** it is refused with the sentence naming why, no subtool is produced,
  and nothing in the answer says the document was touched

#### Scenario: A cage command the layer will not take is an error
- **WHEN** a client asks to raise, drag or apply a cage on a layer that has no
  lattice route, or asks for a cage movement the engine refuses
- **THEN** it is refused with the sentence naming why, and the same sentence is
  on the interface's "why that did not happen" line

#### Scenario: A refused ZSphere or curve command is an error
- **WHEN** a client asks for a rig or curve edit the model refuses
- **THEN** it is refused with the sentence naming why, rather than being
  reported as an edit that happened
