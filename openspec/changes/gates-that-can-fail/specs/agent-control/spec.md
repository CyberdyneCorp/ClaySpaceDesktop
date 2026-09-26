## ADDED Requirements

### Requirement: Consent names the requesting client
The MCP server SHALL remember `clientInfo.name` from each successful initialization for that session. A consent prompt SHALL name the client that made the gated call. For a client that omitted `clientInfo.name`, the prompt SHALL use a localized generic name.

#### Scenario: Two clients request consent
- **WHEN** two initialized clients with different names request gated operations
- **THEN** each prompt SHALL show the name of the client that made that request
