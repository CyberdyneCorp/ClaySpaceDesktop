## Design

Keep a session ownership ledger beside the MCP catalogue. The queue serializes each command and its history read, ownership check, application, and ledger update on the interface thread. Undo and redo move the same owner between ledger stacks. Observed entries that arrived outside an MCP command are attributed to the window. The wire reports the owner of the next undoable entry, not merely the last command sent.

The current history API exposes stack depths but no per-entry identifiers. Depth changes made by the window are reconciled conservatively as window entries. A future engine history entry ID would make attribution exact across same-depth replacement and asynchronous edits.
