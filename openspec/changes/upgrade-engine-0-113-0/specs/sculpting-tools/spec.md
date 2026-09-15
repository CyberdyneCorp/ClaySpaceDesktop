## MODIFIED Requirements

### Requirement: A drag is seen while it is made
A gesture that **replays from its anchor** SHALL be sent to the model on every
pointer move, on every representation.

The threshold that holds a segment back exists because a *stamping* segment
costs a re-mesh of everything it touched, and sending one per pointer move
re-meshes the same neighbourhood repeatedly. That reasoning does not apply to a
replayed gesture: the whole drag is laid down from its anchor each time, so the
work is the same on the first segment and the fortieth, and waiting buys nothing
while costing exactly what a sculptor sees.

Whether a gesture replays SHALL therefore be asked **before** the representation
is asked, and not after it.

#### Scenario: A short field drag
- **WHEN** a field drag travels less than one stamp gap and the pointer is still
  down
- **THEN** the drag has already reached the model, and a live transaction opened
  for that gesture has been given segments to preview

#### Scenario: A short field stamping stroke
- **WHEN** a stamping stroke travels less than one stamp gap
- **THEN** nothing beyond the press's own dab has been sent, because that verb
  does not replay and every segment would cost a re-mesh

## ADDED Requirements

### Requirement: Every Move drag is its own gesture
Each gesture SHALL be given a name no other gesture in the process has used, and
both Move doors — the live transaction and the held drag — SHALL send that name
with every grab they write.

The engine folds a grab into the one leading an item's chain when both belong to
the drag in progress, and the fold replaces the earlier grab. Unnamed, it decides
by centre and radius compared bit for bit, which cannot tell a drag continuing
from a second drag pressed at the same point at the same size.

The live transaction cannot carry the name on ClayCore v0.113.0, whose
`clay_sdf_move_begin` does not read `gesture_id`; the scenario below holds the
held drag until a pin carries that fix.

#### Scenario: A second drag from the same press
- **WHEN** a held Move drag is made, released, and a second is made from the
  same press point at the same brush size
- **THEN** the item carries one grab more than after the first drag, and the
  surface has moved further than the first drag left it

#### Scenario: One drag sent in segments
- **WHEN** a Move drag is sent to the model in several segments
- **THEN** the item carries one grab for the whole drag
