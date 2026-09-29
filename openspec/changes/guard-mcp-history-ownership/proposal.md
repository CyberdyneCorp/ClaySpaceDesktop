## Why

The undo stack is shared by all MCP clients. A client can undo another client's work without knowing an intervening edit occurred. Issue #194 also identified destructive operations whose recovery policy was unclear and a regional refinement route claimed as complete despite being unreachable.

## What Changes

- Attribute undoable entries to the MCP session that created them and refuse a different session's undo or redo with the owner's name.
- Report the next undoable entry's owner in `state.history`.
- Classify every layer operation by its history or consent requirement. Keep regional refinement unoffered until it has a safe route.
- Correct the archived task's completion claim.

## Impact

MCP clients can inspect ownership before stepping history. Existing app commands and undo records remain in the same order.
