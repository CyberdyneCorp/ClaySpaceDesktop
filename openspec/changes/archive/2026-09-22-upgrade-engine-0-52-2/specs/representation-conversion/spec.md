## MODIFIED Requirements

### Requirement: A conversion adds a layer, or replaces the one it read
The application SHALL produce a new layer from a conversion and SHALL leave the
source layer unchanged by default, so that a crossing can be reconsidered
without redoing the work that led to it.

The application SHALL also offer a conversion **in place**: the source layer
leaves as the result arrives, and the result takes the source's row in the
stack. The interface SHALL state which of the two a crossing will do before it
runs, and adding SHALL be the default, since it is the one that cannot lose
work.

A crossing SHALL be one undo step either way, and the depth the interface
reports SHALL count it as one. An in-place crossing leaves more than one engine
entry — the removal and the reorder are recorded separately and an undo group
does not swallow them — so the application SHALL record how many it left and
step over all of them together.

That step SHALL take back both the layer the crossing added and the content
that filled it, and a redo SHALL put both back. Removing the layer SHALL remain
a way to take back a crossing that added one, since the source layer surviving
is what makes that sufficient.

This reverses what this requirement once said, and the reason it said it. On
engine v0.39.0 a conversion produced no undo entry — layer creation and
rasterization were not recorded — so there was genuinely nothing for undo to
take back, and an application-side history was a larger claim than the
operation was worth. Since the engine's `unify-the-undo-history`, the filling
*is* recorded and layer creation still is not, so an engine undo empties the
new layer and leaves it standing. Measured across the pin, one undo of the same
crossing: v0.39.0 left the layer's 3,952 vertices alone, v0.52.2 left it in the
list at zero. An empty layer nobody asked for is not "taken back", so the
application carries the layer while the engine carries the filling.

The layer SHALL be taken off the scene rather than removed from the document
while a crossing is undone. This is forced rather than preferred: the engine
records the inverse of every edit, removal included, so removing the layer
would itself be an undo step — measured, a second undo brought the emptied
layer back and a redo then built a third layer beside it. A saved file SHALL
NOT contain the layer of an undone crossing, since a file carries no redo stack
to put its content back from.

#### Scenario: The source survives a crossing that adds
- **WHEN** a conversion completes without being asked to replace
- **THEN** the source layer is still present with its content unchanged

#### Scenario: A crossing in place replaces the layer it read
- **WHEN** a conversion is run in place
- **THEN** the source layer is gone, the result stands in the row the source
  held, and the stack is no taller than before

#### Scenario: One undo takes a crossing back
- **WHEN** the user undoes once after a crossing
- **THEN** the document holds what it held before it, including the source
  layer where the crossing replaced one

#### Scenario: Undoing past a crossing does not put it back
- **WHEN** the user undoes twice immediately after a conversion
- **THEN** the second undo takes back the edit before the conversion
- **AND** the converted layer is still not in the scene

#### Scenario: A crossing comes back filled
- **WHEN** the user redoes an undone conversion
- **THEN** the converted layer is in the scene again
- **AND** it holds the content the conversion produced, not an empty layer

#### Scenario: A crossing that added a layer is taken back by removing it
- **WHEN** the user removes the layer a conversion added
- **THEN** the document holds what it held before the conversion

#### Scenario: An undone crossing is not saved
- **WHEN** a document is saved while a conversion is undone
- **THEN** the saved file does not contain the layer that conversion produced
