## ADDED Requirements

### Requirement: MCP history entries have an owner
The MCP door SHALL attribute each undoable entry to the session that made it. It SHALL refuse undo or redo when the next entry belongs to another session, naming that owner. `state.history` SHALL report the owner of the next undoable entry. Entries observed without an MCP owner SHALL be attributed to the window.

#### Scenario: A second client cannot undo the first client's edit
- **WHEN** one MCP session adds a layer and another session asks to undo
- **THEN** the undo is refused, the first session is named, and the layer remains

#### Scenario: Ownership follows the history stack
- **WHEN** a client undoes its own entry above another client's entry
- **THEN** the next undo is reported as belonging to the other client
