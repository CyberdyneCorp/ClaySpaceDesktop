## ADDED Requirements

### Requirement: A gated operation an agent asks for completes without a dialog
Saving to a path, opening a document, importing a mesh and exporting one SHALL
each take the path as an argument over the door, and once consent is given
SHALL complete with no file panel opened. The person SHALL be shown that path in
the ask. The file-panel forms of these commands SHALL stay on the pointer path
and SHALL be listed as not offered.

A call over the door SHALL NOT open a native dialog. Where the pointer's path
would stop to ask the person — where to save a document that has never been
saved, whether unsaved work may be lost on an open or a quit, what becomes of a
dragged cage when the active layer changes — the door SHALL refuse the call,
changing nothing, and SHALL name the call that gives the answer up front.

`layer.select` SHALL accept `cage` as `apply` or `discard`. With it, a dragged
cage standing on the active layer is applied or taken down before the switch;
where it is still standing afterwards, the switch does not happen.

#### Scenario: An export takes its path
- **WHEN** an agent calls `exchange.run_export` with a path and the person
  agrees
- **THEN** the mesh is written to that path and no file panel opens

#### Scenario: Save as is offered
- **WHEN** an agent calls `document.save_as` with a path and the person agrees
- **THEN** the document is written there and becomes the document's own path

#### Scenario: An export without a path is refused
- **WHEN** an agent calls `exchange.run_export` without a path
- **THEN** it is refused as a bad argument and nobody is asked

#### Scenario: A standing cage is settled by an argument
- **WHEN** an agent selects another layer while a dragged cage stands
- **THEN** without `cage` the call is refused naming the argument, and with
  `cage: "apply"` the cage is applied and the layer selected, with no prompt

#### Scenario: Unsaved work is not consented away by an open
- **WHEN** an agent opens a document while the open one has unsaved work
- **THEN** the call is refused saying to save it or to start `document.new`,
  and no prompt opens

### Requirement: The wait for a consent fits inside the call bound
The time a call waits for the person to answer an ask SHALL be shorter than the
bound on one call, so a client bounded the way the server bounds its own work
hears back before it gives up.

Where the wait ends unanswered, the call SHALL be refused saying the ask is
still standing, and the ask SHALL stay up at the window. A retry of the same
operation SHALL pick up an answer given in the meantime rather than raising a
new ask.

#### Scenario: A consented operation does not time out
- **WHEN** a person agrees to an ask after the first call's wait has ended
- **THEN** the retried call completes with that agreement

#### Scenario: A long configured wait is still held inside the call
- **WHEN** the consent wait is configured longer than the call bound
- **THEN** an unanswered ask is refused within the call bound
