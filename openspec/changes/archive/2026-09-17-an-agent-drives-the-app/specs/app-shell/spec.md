## MODIFIED Requirements

### Requirement: The status area reports document, memory and backend state
The status area SHALL display the current document name and modified state, the working unit, the memory in use against the configured budget, the active evaluation backend, and whether the application is listening for an agent.

The listening indicator SHALL say whether a client is currently connected, and
SHALL show when an agent last changed the document. A surface that moved while
nobody touched the window is otherwise a defect report with no cause in it.

#### Scenario: Memory reflects the engine's own accounting
- **WHEN** memory usage is displayed
- **THEN** the figures come from the engine's brick cache statistics and budget, not from an estimate maintained by the application

#### Scenario: Approaching the budget is visible before it is reached
- **WHEN** memory in use approaches the configured budget
- **THEN** the indicator changes state before the budget is exhausted, rather than only at failure

#### Scenario: Listening is visible
- **WHEN** the application is listening for an agent
- **THEN** the status area says so, and says whether a client is connected

#### Scenario: A change made by an agent is attributable
- **WHEN** an agent changes the document
- **THEN** the status area records that an agent acted and when

## ADDED Requirements

### Requirement: The person at the window controls the door
The interface SHALL offer, from the menu bar, the means to stop the server, to
start it again, and to see the address and secret a client would need.

The choice SHALL be remembered in the session store: a server stopped by hand
SHALL stay stopped when the application is opened again, and one started by
hand SHALL start with it.

Where a gated operation asks for the person's consent, the request SHALL be
presented in the interface saying what is being asked for, by which client, and
on which path where a path is involved, and SHALL be refusable. Consent SHALL
be for the operation asked for, and SHALL NOT stand for later ones unless the
person records an opt-in for that kind of operation.

#### Scenario: The server is stopped from the menu
- **WHEN** the person stops the server from the menu bar
- **THEN** it stops listening, existing connections are closed, and the choice
  survives reopening the application

#### Scenario: The person can find the address
- **WHEN** the person asks how to connect a client
- **THEN** the interface shows the address and the secret a client would need

#### Scenario: Consent names what is being asked
- **WHEN** an agent asks for an operation that would write over a file
- **THEN** the request presented names the operation and the path, and can be
  refused

#### Scenario: Consent does not generalise
- **WHEN** the person agrees to one export and the agent asks for a second
- **THEN** the second is asked for again, unless an opt-in for exports has been
  recorded
