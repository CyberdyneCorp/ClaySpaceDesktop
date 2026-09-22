## ADDED Requirements

### Requirement: Writing a file does not change what is drawn
Saving a document SHALL NOT alter the scene on the screen. A save is not an
edit: no layer's visibility as the viewport reads it SHALL move, no part of the
surface SHALL be re-evaluated, and nothing SHALL be left for the sculptor to
undo.

This binds the one place the file and the screen legitimately disagree. A solo
is a way of looking at a document rather than part of it, so a file written
while one is engaged SHALL record the visibility the sculptor set — a document
that reopened with everything but one subtool hidden, the crash recovery
included, would carry a way of looking at it as though it were the work. The
pattern SHALL reach the file without reaching the live scene: what is written
is the sculptor's, what stays drawn is the solo.

Where the engine records a command for each flag a save has to write, those
commands SHALL be stepped over as one, so that the undo after a save reaches
the sculptor's own edit and not the file's bookkeeping.

#### Scenario: A soloed document is saved without redrawing
- **WHEN** a document with a subtool soloed is saved
- **THEN** no brick of the surface is re-evaluated, and the viewport goes on
  showing the soloed subtool alone

#### Scenario: The file carries the sculptor's own pattern
- **WHEN** a document saved while soloed is reopened
- **THEN** every layer's visibility is the one the sculptor set before the
  solo, and the reopened document is not soloed

#### Scenario: A save costs no undo step
- **WHEN** a sculptor edits, engages a solo, saves, and undoes
- **THEN** the edit is what is taken back

## MODIFIED Requirements

### Requirement: Unsaved work survives a crash
The application SHALL autosave recovery state for open documents at a configurable interval and after significant edits. On starting after an abnormal termination, it SHALL offer to recover each document that has recovery state newer than its saved file.

The interval SHALL be idle time between autosaves: it is counted from the
moment one autosave finishes rather than the moment it starts. Counted from one
start to the next, a document whose save takes longer than the interval is due
again the instant it lands, and the application spends its time saving instead
of being usable between saves.

A failed autosave SHALL wait out the same interval as a successful one, so that
a save that cannot be written is not retried on every turn of the event loop.

An autosave SHALL be skipped while a gesture is open — a stroke, a manipulator
drag, an outline — and SHALL NOT thereby be cancelled: skipping a tick SHALL
NOT restart the interval, so the write happens as soon as the hand comes off.

#### Scenario: Recovery is offered after a crash
- **WHEN** the application starts after terminating abnormally with unsaved changes
- **THEN** it lists the recoverable documents and lets the user open or discard each

#### Scenario: Autosave does not overwrite the user's file
- **WHEN** autosave runs on a document with unsaved changes
- **THEN** recovery state is written separately and the user's saved file is unchanged

#### Scenario: A slow autosave is not immediately due again
- **WHEN** an autosave takes longer than the configured interval
- **THEN** the next one is not due until the interval has passed since it
  finished

#### Scenario: An autosave that fails does not retry on every wake-up
- **WHEN** an autosave cannot be written
- **THEN** the next attempt is an interval away, as it would be after a
  successful write

#### Scenario: A gesture holds the autosave off and does not cancel it
- **WHEN** an autosave falls due while a stroke, drag or outline is open
- **THEN** nothing is written until the gesture ends, and it is written then
  rather than an interval later

#### Scenario: Clean exit clears recovery state
- **WHEN** the application exits normally with all documents saved
- **THEN** no recovery is offered on the next start
