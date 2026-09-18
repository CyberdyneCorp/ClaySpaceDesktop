## ADDED Requirements

### Requirement: A smoothing stroke is shown while it is being made
A stroke with a region tool on a field SHALL be shown on the surface as it is
made, rather than only when the pointer is released, wherever the engine can
hold the gesture open.

While such a gesture is open the document SHALL NOT change: no items, no
deformers and no history entries until the gesture is laid down.

The surface SHALL come to rest where the preview showed it. The preview and
the result need not be the same computation — the preview may be produced by
machinery the final edit does not use — but the difference SHALL be smaller
than a sculptor can see, and SHALL be measured rather than assumed.

Laying the gesture down SHALL NOT degrade the rest of the subtool. In
particular the application SHALL NOT consolidate a whole layer as a side
effect of a stroke that touched part of it.

A gesture that cannot be shown live SHALL fall back to being held whole and
applied when the pointer is released, which is correct but not live. The
application SHALL NOT draw a preview it cannot compose correctly — in
particular where a second visible field subtool shares the surface being
drawn.

The previewed surface SHALL be meshed by the engine from samples the engine
computed. The application SHALL NOT interpolate, resample or otherwise decide
where the previewed surface lies.

#### Scenario: The surface moves while the pointer is down
- **WHEN** the user drags the smoothing brush across the form
- **THEN** the surface the viewport draws changes before the stroke ends

#### Scenario: Nothing is written down until the stroke ends
- **WHEN** a live gesture is open and dabs have been applied
- **THEN** the document's history is unchanged, and abandoning the gesture
  leaves the surface exactly as it was

#### Scenario: The result lands where the preview showed it
- **WHEN** a live gesture is laid down
- **THEN** the surface stands where the preview showed it, within a stated
  tolerance, rather than visibly moving when the gesture ends

#### Scenario: A stroke does not re-bake the whole subtool
- **WHEN** the user smooths part of a subtool
- **THEN** the rest of it is left as it was, at the resolution it had

#### Scenario: A second field subtool falls back
- **WHEN** a second field subtool is visible and the user smooths
- **THEN** the gesture is held whole and applied when the pointer is released,
  as it was before

#### Scenario: A live gesture is one action to undo
- **WHEN** a live gesture is committed and the user undoes once
- **THEN** the whole gesture is taken back, however many dabs drew it

### Requirement: A gesture in progress is previewed without erasing itself
While a gesture is open, the model SHALL take back what its last segment did
only for verbs that are delivered again from their anchor on every segment. A
verb delivered as the samples that are new SHALL have its record *continued*
rather than replaced, so that a drag builds up as it is drawn.

Either way the gesture SHALL remain one undo, and taking it back SHALL put
every vertex where it was.

#### Scenario: A stamping drag builds up
- **WHEN** the user drags a stamping brush across a mesh in one gesture
- **THEN** every dab of the drag is on the surface when the gesture ends, not
  only the last

#### Scenario: A drag is one undo
- **WHEN** the user undoes a mesh drag once
- **THEN** the whole drag is taken back, however many segments drew it, and
  every vertex is where it was
