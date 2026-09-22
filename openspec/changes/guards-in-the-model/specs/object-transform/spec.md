## MODIFIED Requirements

### Requirement: A cage owns the pointer, cursor and all
While a deformation cage is up, the brush cursor SHALL NOT be drawn over the
form. A ring under the pointer states that the next press leaves a stroke, and
while a cage is up no press does.

The rule SHALL be one rule covering every mode that takes the press away from
the brush — the whole-subtool manipulator and the cage alike.

A raised cage SHALL refuse a stroke on **every** path and not only under the
pointer. The refusal SHALL be stated where the command is applied, so that a
caller which never touched a pointer meets it too, and SHALL name the cage so
the caller knows to apply it or take it down. The pointer's own routing SHALL
remain as a second line rather than the only one.

A raised cage SHALL also take the whole-subtool manipulator's target away. The
cage and the placed object both answer the manipulator's commands and which of
them acts is decided by which one has a target, so a target left standing under
a cage makes one drag change two things.

#### Scenario: No brush is drawn while a cage is up
- **WHEN** a deformation cage is up and the pointer is over the form
- **THEN** no brush cursor is drawn, and a press leaves no stroke

#### Scenario: A stroke asked for while a cage is up is refused
- **WHEN** a stroke is begun on a caged layer by a caller that is not the
  pointer
- **THEN** it is refused naming the cage, nothing is collected, and the layer
  is unchanged

#### Scenario: A cage drag moves only the cage
- **WHEN** a cage is raised over a selected object and the manipulator is
  dragged
- **THEN** the cage's control points move and the object does not
