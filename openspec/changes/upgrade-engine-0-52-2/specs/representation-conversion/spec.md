## MODIFIED Requirements

### Requirement: A conversion produces a new layer and is one undo step
The application SHALL produce a new layer from a conversion and SHALL leave the
source layer unchanged, so that a crossing can be reconsidered without redoing
the work that led to it.

A conversion SHALL be undoable as a single step, taking back both the layer the
crossing added and the content that filled it, and a redo SHALL put both back.
Removing the layer SHALL remain a way to take a crossing back, since the source
layer surviving is what makes that sufficient.

This reverses what this requirement said, and the reason it said it. On engine
v0.39.0 a conversion produced no undo entry — layer creation and rasterization
were not recorded — so there was genuinely nothing for undo to take back, and
an application-side history was a larger claim than the operation was worth.
Since the engine's `unify-the-undo-history`, the filling *is* recorded and
layer creation still is not, so an engine undo empties the new layer and leaves
it standing. Measured across the pin, one undo of the same crossing: v0.39.0
left the layer's 3,952 vertices alone, v0.52.2 left it in the list at zero. An
empty layer nobody asked for is not "taken back", so the application now
carries the layer while the engine carries the filling.

The layer SHALL be taken off the scene rather than removed from the document
while a crossing is undone. This is forced rather than preferred: the engine
records the inverse of every edit, removal included, so removing the layer
would itself be an undo step — measured, a second undo brought the emptied
layer back and a redo then built a third layer beside it. A saved file SHALL
NOT contain the layer of an undone crossing, since a file carries no redo stack
to put its content back from.

#### Scenario: The source survives
- **WHEN** a conversion completes
- **THEN** the source layer is still present with its content unchanged

#### Scenario: A crossing is taken back by one undo
- **WHEN** the user undoes immediately after a conversion
- **THEN** the layer the conversion produced is no longer in the scene
- **AND** the undo history is at the depth it was at before the conversion

#### Scenario: Undoing past a crossing does not put it back
- **WHEN** the user undoes twice immediately after a conversion
- **THEN** the second undo takes back the edit before the conversion
- **AND** the converted layer is still not in the scene

#### Scenario: A crossing comes back filled
- **WHEN** the user redoes an undone conversion
- **THEN** the converted layer is in the scene again
- **AND** it holds the content the conversion produced, not an empty layer

#### Scenario: A conversion is taken back by removing its layer
- **WHEN** the user removes the layer a conversion produced
- **THEN** the document holds what it held before the conversion

#### Scenario: An undone crossing is not saved
- **WHEN** a document is saved while a conversion is undone
- **THEN** the saved file does not contain the layer that conversion produced
