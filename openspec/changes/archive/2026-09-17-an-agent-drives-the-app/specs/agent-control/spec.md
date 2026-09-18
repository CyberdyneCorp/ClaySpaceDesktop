## Purpose

Lets a program outside the application drive the session a sculptor is already
in — through the same commands the interface emits, over a channel that only
this machine can reach, with the operations that can destroy work held behind a
consent the channel itself cannot supply.

## ADDED Requirements

### Requirement: The application listens for as long as it is open
The application SHALL run a Model Context Protocol server for the whole of its
lifetime, started as part of opening and stopped as part of closing, so that a
client can reach a session that was already open rather than having to start
one.

The server SHALL bind to the loopback interface only. It SHALL NOT bind to any
address reachable from another machine, and a configuration that asks it to
SHALL be refused rather than honoured.

Where the preferred port is already taken, the server SHALL take another rather
than failing to start, and SHALL publish the port it actually took. A second
copy of the application running at the same time SHALL therefore be reachable
too, each on its own port, each identifying which document it holds.

The application SHALL remain fully usable if the server cannot start at all. A
port that cannot be bound or a session directory that cannot be written costs
the door, not the ability to sculpt, and SHALL be reported where errors are
reported rather than ending the process.

#### Scenario: A client reaches a session already open
- **WHEN** the application has been open and a client connects to the published
  address
- **THEN** it is served against the document currently open, with no new
  application process started

#### Scenario: The door is loopback only
- **WHEN** the server's listening address is inspected
- **THEN** it is on the loopback interface, and a connection attempt from
  another host does not reach it

#### Scenario: Two sessions at once
- **WHEN** a second copy of the application is opened while the first is
  running
- **THEN** it takes a different port, publishes it, and each session is
  reachable separately and says which document it holds

#### Scenario: The door fails and the application does not
- **WHEN** the port cannot be bound
- **THEN** the application opens, sculpting works, and the failure is reported
  with its cause

### Requirement: A client proves it is on this machine before it is served
The server SHALL require a bearer secret on every request, and SHALL publish
that secret only in a file in the per-user session directory whose permissions
allow the owner alone to read it.

A request without the secret, or with the wrong one, SHALL be refused without
disclosing whether a session exists, what document is open, or what the correct
secret looks like.

The secret SHALL be new for each run of the application, and SHALL be removed
when the application closes, so that a secret read once does not grant access
to a later session.

The server SHALL reject a request whose declared origin is not one it issued
its own address for, so that a page in a browser cannot reach the session
through a name that resolves to loopback.

#### Scenario: A client with the secret is served
- **WHEN** a client reads the session directory and presents the secret
- **THEN** its requests are served

#### Scenario: A client without the secret is refused
- **WHEN** a request arrives with no secret or a wrong one
- **THEN** it is refused, and the refusal says nothing about the session behind
  it

#### Scenario: The secret does not outlive the session
- **WHEN** the application closes and is opened again
- **THEN** the previous secret no longer authenticates, and the published file
  holds the new one

#### Scenario: A browser page is not a client
- **WHEN** a request arrives declaring a web origin the server did not issue
- **THEN** it is refused regardless of the secret it carries

### Requirement: Every tool dispatches the command path the interface dispatches
A tool that changes anything SHALL do so by dispatching the same command the
interface dispatches for that change. The server SHALL NOT hold a second way to
mutate the document, the scene or application state.

A change made through a tool SHALL therefore be indistinguishable afterwards
from the same change made by hand: one entry in the edit history where the
interface would make one, undone by the same undo, reported by the same
observable state, and subject to the same conditions that disable the
equivalent control.

Where the interface refuses a change, the tool SHALL be refused for the same
reason and in the same words, rather than reaching past the refusal.

#### Scenario: A tool and a click are the same edit
- **WHEN** a stroke is applied through a tool and the same stroke is applied by
  hand
- **THEN** both produce one history entry, and one undo returns the document in
  both cases

#### Scenario: A tool cannot reach past a refusal
- **WHEN** a tool asks for an operation the interface would refuse — a
  deformation cage on a stretched subtool, an operation disabled for the active
  representation
- **THEN** it is refused with the reason the interface would give

#### Scenario: Every mutation is still observable in one place
- **WHEN** the command path is instrumented and an agent drives the application
- **THEN** every change it makes appears in that instrumentation, alongside the
  ones a person makes

### Requirement: The tool surface is grouped by feature and describes itself
The server SHALL present its capability as a set of tools grouped by the
application's own domains rather than one tool per command, so that a client
chooses from a list it can hold. Each tool SHALL take the name of an action
within its group and that action's arguments.

The server SHALL offer a way to ask what actions a group holds, what arguments
each takes, which are required, and which of them are currently unavailable and
why — answered from the application's own command vocabulary rather than from a
separately maintained description, so that the answer cannot drift from what
the application will accept.

An unknown action or a malformed argument SHALL be refused with a message
naming what was expected, and SHALL change nothing.

Every group the surface presents SHALL hold at least one action, and every
feature the interface offers SHALL be reachable through some group.

#### Scenario: A client learns the vocabulary at runtime
- **WHEN** a client asks a group what it offers
- **THEN** it receives the actions, their arguments and which are required,
  matching what the application will actually accept

#### Scenario: An unavailable action says why
- **WHEN** an action is asked for that the current representation, selection or
  document state does not allow
- **THEN** the refusal names the condition rather than reporting a generic
  failure

#### Scenario: A malformed call changes nothing
- **WHEN** a tool is called with a missing or wrongly typed argument
- **THEN** it is refused, and the document and application state are unchanged

#### Scenario: Every feature is reachable
- **WHEN** the tool surface is compared against the application's command
  vocabulary
- **THEN** every command the interface can emit is reachable through some
  group, or is listed with the reason it is deliberately not offered

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

### Requirement: What can destroy work is held behind a consent the secret cannot supply
Operations that can lose a person's work SHALL require a consent separate from
the connection's authentication. At minimum this SHALL cover: writing over an
existing file, exporting to a path, opening a document over an unsaved one,
discarding an unsaved document, deleting a subtool or layer that cannot be
recovered by undo, and closing the application.

The consent SHALL be either an opt-in recorded in the session store by the
person who owns the session, or an answer given at the window when the
operation is asked for. Possession of the connection secret alone SHALL NOT be
consent.

Sculpting, masking, transforming, selecting, changing tools, navigating,
undoing and redoing SHALL NOT be gated. They are what the session is for and
they are all recoverable through the edit history.

A gated operation that is refused SHALL name the gate and say what would lift
it, so that an agent can ask the person rather than retrying.

Where the person is asked at the window and does not answer, the operation
SHALL be refused after a bounded wait rather than holding the connection open
indefinitely.

#### Scenario: An export needs consent
- **WHEN** an agent asks to export to a path and no opt-in is recorded
- **THEN** the person is asked at the window, and the export happens only if
  they agree

#### Scenario: Sculpting is not gated
- **WHEN** an agent applies a stroke, a mask and an undo
- **THEN** none of them is gated, and each is one history entry

#### Scenario: A refusal is actionable
- **WHEN** a gated operation is refused
- **THEN** the message names the gate and what would lift it

#### Scenario: An unanswered prompt does not hold the connection
- **WHEN** the person is asked and does not answer
- **THEN** the operation is refused after a bounded wait, saying so

### Requirement: A connected agent does not cost the sculptor a frame
Serving the connection SHALL happen off the interface thread. Only the
application of a command and the capture of a frame SHALL touch it, and each
SHALL be bounded so that neither a slow client nor a request that cannot be
served holds a frame.

A client that connects and then goes quiet, a client that reads its response
slowly, and a client that disconnects mid-request SHALL each cost nothing
measurable in the viewport's frame rate.

Work an agent asks for that outlasts a frame SHALL run the way the same work
runs when a person asks for it — off the interface thread, with progress
observable — rather than blocking either the interface or the answer.

#### Scenario: An idle connection is free
- **WHEN** a client is connected and sends nothing for a minute
- **THEN** the viewport's frame timing is unchanged from having no client at
  all

#### Scenario: A slow client does not stall the window
- **WHEN** a client stops reading its response part-way
- **THEN** the interface continues to redraw and respond

#### Scenario: Long work does not block the answer
- **WHEN** an agent asks for an operation that outlasts a frame
- **THEN** it can observe the operation's progress, and the interface stays
  responsive while it runs
