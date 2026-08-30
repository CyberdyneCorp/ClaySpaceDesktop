## ADDED Requirements

### Requirement: A subtool that has become costly to evaluate says so
A field subtool SHALL report what its edit list costs to evaluate and whether
the engine advises collapsing it, and the interface SHALL offer that collapse
while the advice stands.

The application SHALL NOT collapse a layer on its own. Collapsing costs seconds
and changes what the layer holds, so it is offered and never taken quietly.

Reporting the advice SHALL NOT cost what acting on it would cost to estimate:
the advice is asked for whenever the scene is assembled, and what collapsing
would occupy is asked for only when it is about to be shown to a sculptor who
is deciding.

#### Scenario: The offer appears when the engine advises it
- **WHEN** a field subtool has been worked until the engine advises collapsing it
- **THEN** the subtool panel offers to collapse it, and does not before

#### Scenario: Collapsing is the sculptor's decision
- **WHEN** the engine advises collapsing a subtool
- **THEN** nothing is collapsed until the sculptor asks for it

#### Scenario: The offer goes away once taken
- **WHEN** the user collapses the subtool
- **THEN** the layer reports itself collapsed and the offer is no longer made
