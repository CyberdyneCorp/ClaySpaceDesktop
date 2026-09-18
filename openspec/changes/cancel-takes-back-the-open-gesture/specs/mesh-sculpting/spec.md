## MODIFIED Requirements

### Requirement: A mesh gesture is one undo step
The application SHALL record a mesh sculpting gesture as a single undoable
action that reverts the mesh exactly.

Cancelling a gesture in progress SHALL revert that gesture and nothing
underneath it, whatever the representation records it as. A mesh gesture is
previewed as it is made and banked as one record at the release, so the entries
its segments appear to have produced are not what a cancel owes; what it owes
is the document as it stood when the gesture opened.

#### Scenario: One gesture, one undo
- **WHEN** the user completes a mesh stroke and undoes
- **THEN** the mesh is exactly as it was before the stroke began

#### Scenario: A cancelled gesture leaves the committed ones standing
- **WHEN** the user commits two mesh gestures, begins a third and cancels it
- **THEN** the mesh is exactly as the second gesture left it, and both
  committed gestures are still there to undo

#### Scenario: A cancelled first gesture leaves the layer
- **WHEN** the user begins a gesture on a mesh layer that has just been created
  and cancels it
- **THEN** the layer is still in the document, holding what it held before the
  gesture began
