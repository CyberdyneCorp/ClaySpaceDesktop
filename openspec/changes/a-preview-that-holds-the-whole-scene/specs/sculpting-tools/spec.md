## MODIFIED Requirements

### Requirement: A smoothing stroke is shown while it is being made
A stroke with a region tool on a field SHALL be shown on the surface as it is
made, rather than only when the pointer is released, wherever the engine can
hold the gesture open.

While such a gesture is open the document SHALL NOT change: no items, no
deformers and no history entries until the gesture is laid down. This SHALL
hold for everything the preview does, including reading the rest of the
document in order to draw it.

The surface SHALL come to rest where the preview showed it. The preview and
the result need not be the same computation — the preview may be produced by
machinery the final edit does not use — but the difference SHALL be smaller
than a sculptor can see, and SHALL be measured rather than assumed.

Laying the gesture down SHALL NOT degrade the rest of the subtool. In
particular the application SHALL NOT consolidate a whole layer as a side
effect of a stroke that touched part of it.

**A preview SHALL show the rest of the scene beside the layer it previews.**
Where other field subtools are visible, the application SHALL compose them into
what it draws, and the composition SHALL be the engine's own union rather than
an approximation of it. It SHALL NOT reach that composition by editing the
document — hiding layers around the gesture and showing them again is an edit,
and a gesture that is holding a layer refuses one.

A gesture that cannot be shown live SHALL fall back to being held whole and
applied when the pointer is released, which is correct but not live. The
application SHALL NOT draw a preview it cannot compose correctly.

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

#### Scenario: A second field subtool is still drawn
- **WHEN** a second field subtool is visible and the user smooths the first
- **THEN** the gesture is shown live, and the second subtool is on screen for
  the whole of it

#### Scenario: Reading the rest of the scene does not spoil the gesture
- **WHEN** a live gesture composes the rest of the document into its preview,
  on every segment
- **THEN** the history depth does not move while the gesture is open, and the
  gesture still commits when the pointer is released

#### Scenario: A hidden or empty subtool is neither drawn nor a refusal
- **WHEN** the other field subtool in the document is hidden, or has nothing
  in it
- **THEN** the gesture opens, and what is drawn is what would be drawn with
  that subtool absent

#### Scenario: A live gesture is one action to undo
- **WHEN** a live gesture is committed and the user undoes once
- **THEN** the whole gesture is taken back, however many dabs drew it
