## MODIFIED Requirements

### Requirement: A tool call is applied before it answers
A tool that changes something SHALL return only after that change has been
applied on the interface thread. It SHALL NOT return on having queued the
change.

Calls SHALL be applied in the order they arrived, including across more than
one connected client, so that two agents driving one session cannot interleave
within a single call.

A request arriving while the application is idle SHALL wake it. A session
waiting for input SHALL NOT have to be moved by a person before an agent's
command takes effect.

While a person is holding a gesture — a stroke, a manipulator drag, a mask
outline — a tool that would change the document SHALL be refused and SHALL say
that a gesture is in progress. Tools that only read SHALL be served during a
gesture.

While the **agent itself** is holding a gesture it opened, only the verbs that
carry that gesture on or close it SHALL be applied. Every other tool that would
change the document SHALL be refused, naming the open gesture. Opening a second
gesture on top of an open one SHALL be refused on the same ground: the engine
holds one gesture at a time.

For both rules, "would change the document" SHALL be the wide question and not
the edit history's narrower one. An operation that marks the document on a path
of its own rather than through the ordinary edit path — a crossing, an import,
a pass of the active layer's stack — SHALL be refused during a gesture like any
other change.

A gesture a caller opened that the application refused SHALL NOT count as a
gesture in progress. The record of whose gesture is open states an intent
before the command is applied, and an intent that opened nothing SHALL NOT
stand.

#### Scenario: The answer means it happened
- **WHEN** a tool that inserts a subtool returns
- **THEN** the subtool is in the document and visible to any other reader of
  the session

#### Scenario: An idle session still answers
- **WHEN** a request arrives at an application that has had no input for
  minutes
- **THEN** it is applied and answered without a person touching the window

#### Scenario: Two clients do not interleave
- **WHEN** two connected clients call changing tools at the same moment
- **THEN** each call is applied whole, in arrival order

#### Scenario: A person's gesture is not interrupted
- **WHEN** a changing tool is called while a stroke is being drawn by hand
- **THEN** it is refused, saying a gesture is in progress, and the stroke is
  unaffected

#### Scenario: An agent may finish the stroke it opened
- **WHEN** an agent that has begun a stroke calls continue, end or cancel
- **THEN** each is applied, and the stroke becomes one entry in the history

#### Scenario: An agent may do nothing else inside its own stroke
- **WHEN** an agent that has begun a stroke calls a structural tool — adding a
  layer, rebuilding one, undoing, crossing a representation
- **THEN** it is refused naming the open gesture, and the document is unchanged

#### Scenario: A refused begin does not wedge the door
- **WHEN** an agent's stroke is refused by the application and the agent then
  calls a changing tool
- **THEN** the call is applied, because no gesture was ever opened
