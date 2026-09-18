## MODIFIED Requirements

### Requirement: Layers are presented as an ordered stack
The application SHALL present layers as an ordered stack, showing each layer's name, visibility, protection state and intensity, with the evaluation order matching the engine's ordered edit list. The user SHALL be able to create, rename, reorder and remove layers.

Reordering SHALL be reachable from the stack itself — a row is dragged to its
new position — and SHALL dispatch the same command path every other layer
mutation dispatches, so that it is one undo step and so that it reaches the
agent door by existing rather than by a second route written for it.

A layer stack's order is its evaluation order, which a hierarchy's pass order is
not. The drop target SHALL therefore say what the new order evaluates to before
the drop, rather than leaving the sculptor to read it off the surface afterwards.

#### Scenario: Reordering changes evaluation order
- **WHEN** the user moves a layer above another
- **THEN** the document is re-evaluated with the new order and the viewport reflects the result

#### Scenario: Reordering is one undo step
- **WHEN** the user drags a layer to a new position and undoes once
- **THEN** the stack is in the order it was, in one step

#### Scenario: Removing a layer is undoable
- **WHEN** the user removes a layer and undoes the removal
- **THEN** the layer returns with its content, position in the stack, and settings intact

### Requirement: Layer protection states are distinct and enforced
The application SHALL expose the engine's three protection states: visible, ghosted (shown, not pickable, not editable) and locked (shown, pickable, not editable). Attempting to edit a protected layer SHALL be refused with a stated reason rather than silently ignored.

Exposing them means **offering** them, not only displaying them. All three SHALL
be reachable from the layer's own row, which SHALL show which one holds. A badge
drawn on the rows that are already protected, with nothing that sets one, leaves
a sculptor able to see a state they cannot reach — and that is what this
requirement went five months describing without anyone noticing, because the
state was reachable from the ViewModel and from nowhere a person or an agent
stands.

Setting a protection state SHALL dispatch the same command path every other
layer mutation dispatches, and SHALL be one undo step.

#### Scenario: A ghosted layer is not picked
- **WHEN** the user clicks on geometry belonging to a ghosted layer
- **THEN** the pick passes through to whatever is behind it, and the ghosted layer is not selected

#### Scenario: Editing a locked layer is refused with a reason
- **WHEN** the user applies a brush to a locked layer
- **THEN** no edit occurs and the interface states that the layer is locked

#### Scenario: Protection is set from the row that shows it
- **WHEN** the user sets a layer to ghosted from its row in the stack and then
  back to visible
- **THEN** the row shows each state as it holds, and the layer picks again once
  it is visible

#### Scenario: An agent reaches protection because the interface does
- **WHEN** an agent sets a layer's protection through the tool surface
- **THEN** the change is the one the interface makes, undone by the same undo
  and reported by the same observable state
