## MCP client identity

The HTTP session table stores an optional client name beside each opaque session ID. `initialize.params.clientInfo.name` is bounded before storage. Each later request resolves its own session ID and passes that name to the protocol's tool call. The tool surface receives this request context only for the call, so concurrent clients cannot overwrite one shared global name. Catalogue consent uses that name; legacy clients without a session ID or `clientInfo` get the locale's generic agent name.

## Localized notices

The string table remains the owner of visible wording. Model and engine failures cross into the application as structured causes where possible; the application formats a locale-specific sentence and suppresses raw ABI names. Source lint protects known user-visible sinks from new literals outside the string table. Diagnostics and export warnings use the same locale selection as the shell.

Export warnings retain a typed cause beside the canonical English message used on the agent wire. The shell formats the cause for its locale. Legacy ViewModel notice channels still carry prose; the application selects a translated action category at the shell and agent boundaries until those channels carry typed causes. It preserves the original detail in Portuguese sessions when the detail has no raw ABI symbol.

MCP metadata and memory part keys use canonical English terms. The client name is the name declared by `initialize.clientInfo`, bounded and scoped to the session; the protocol does not attest that name.
