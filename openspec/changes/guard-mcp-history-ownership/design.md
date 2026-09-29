## Design

Keep a session ownership ledger beside the MCP catalogue. The queue serializes each command and its history read, ownership check, application, and ledger update on the interface thread. Undo and redo move the same owner between ledger stacks. Observed entries that arrived outside an MCP command are attributed to the window. The wire reports the owner of the next undoable entry, not merely the last command sent.

The document exposes a monotonic history revision as well as stack depth. A revision change at unchanged depth distinguishes a new entry that evicted the oldest from a command that changed nothing. Edits observed outside an MCP call are attributed to the window, including same-depth replacement. The revision is document-wide; a future per-entry ID would allow exact attribution of every entry after several unobserved external edits.
