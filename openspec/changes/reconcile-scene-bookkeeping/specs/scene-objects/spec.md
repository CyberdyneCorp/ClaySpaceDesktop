## ADDED Requirements

### Requirement: The object table follows live document nodes
Every listed object SHALL refer to a live node in an existing document layer. Optimization and conversion SHALL reconcile the table in the same history entry as the document change.

#### Scenario: Optimize folds an object node
- **WHEN** layer optimization removes an object's node
- **THEN** the object is removed from the table, and undo restores both

### Requirement: Object selection names a listed object
A selection of an unknown object SHALL be refused without changing the previous selection.

#### Scenario: Unknown object selection
- **WHEN** an object selection names an unknown node
- **THEN** the selection is refused and the previous selection remains unchanged
