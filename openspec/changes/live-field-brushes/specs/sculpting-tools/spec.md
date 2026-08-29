## ADDED Requirements

### Requirement: A smoothing stroke is shown while it is being made
A stroke with a region tool on a field SHALL be shown on the surface as it is
made, rather than only when the pointer is released, wherever the engine can
hold the gesture open.

While such a gesture is open the document SHALL NOT change: no items, no
deformers and no history entries until the gesture is committed. What the
preview showed SHALL be what the commit installs.

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

#### Scenario: The preview is what is committed
- **WHEN** a live gesture is committed
- **THEN** the surface stands where the preview showed it, rather than moving
  when the gesture ends

#### Scenario: A second field subtool falls back
- **WHEN** a second field subtool is visible and the user smooths
- **THEN** the gesture is held whole and applied when the pointer is released,
  as it was before

#### Scenario: A live gesture is one action to undo
- **WHEN** a live gesture is committed and the user undoes once
- **THEN** the whole gesture is taken back, however many dabs drew it
