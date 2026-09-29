## ADDED Requirements

### Requirement: Agent history reports its next entry owner
The agent state history section SHALL include `last_entry_by` when undo can move an entry and SHALL omit it when history is empty. The value SHALL name the client that made the entry, or `window` for an entry observed outside the MCP door.

#### Scenario: State identifies another client's work
- **WHEN** a second client reads history after the first client edits
- **THEN** `last_entry_by` names the first client
