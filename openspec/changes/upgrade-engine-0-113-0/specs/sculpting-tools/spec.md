## ADDED Requirements

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

<!-- "Every Move drag is its own gesture" stood here. It asked for a name on
     every grab both Move doors write, and held its second scenario to the held
     drag because ClayCore v0.113.0's `clay_sdf_move_begin` did not read
     `gesture_id`. v0.116.0 carries that fix, so the rule is now part of "A drag
     costs the field the gesture, not the segments" in the living
     `sculpting-tools` spec, covering both doors. Keeping a second copy here
     would leave two texts for one rule, with nothing to say which the
     application obeys. -->
