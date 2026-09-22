## ADDED Requirements

### Requirement: An operation on a grid's passes is one step of the history the interface reads
The history a user steps through is the one the application keeps, which counts
**actions** and remembers how many entries each action spent. Dialling a
recorded pass, showing or hiding one and moving one within the stack each SHALL
bank exactly one action on that history, and one step back SHALL put the stack
and the surface where the operation found them.

The engine records nothing for any of these: composing a grid from its passes
is a replay of recorded cells, and a replay is not an edit. The application
SHALL therefore keep the way back itself, SHALL order it against the engine's
own entries by the document's history sequence rather than by a depth, and SHALL
hold both directions — a pass operation is not its own inverse.

Beginning and ending a recording decide where the next edits are filed and draw
nothing new, so they SHALL bank nothing.

An operation that discards the record it acts on — removing a pass, merging one
down — cannot be put back and SHALL NOT bank an action for one. It SHALL also
let go of the ways back into that grid's stack, rather than leave a record
addressing a pass by a position the operation has renumbered.

#### Scenario: Dialling a pass is one thing to take back
- **WHEN** the user changes a recorded pass's strength
- **THEN** the history offers exactly one more thing to undo than it did before

#### Scenario: One undo after dialling a pass takes back the dialling
- **WHEN** the user records a pass over a grid, dials it down and undoes once
- **THEN** the strength is what it was, the surface is what the pass left, and
  the subtools are the ones the document had

#### Scenario: A redo puts the dialling back
- **WHEN** the user takes a pass operation back and redoes
- **THEN** the stack is as the operation left it

#### Scenario: Opening a recording is not something to take back
- **WHEN** the user begins and ends a recording without stroking
- **THEN** the history offers exactly what it did before

#### Scenario: A pass and an engine edit come apart in the order they were made
- **WHEN** the user dials a pass, adds a subtool, hides a pass and undoes three
  times
- **THEN** each undo lands on the moment before the command it was meant for

### Requirement: Rebuilding a mesh layer's topology is one step of the history the interface reads
A rebuild is one engine entry — capture, rebuild, validate, replace, record —
and the application SHALL bank exactly one action for it on the history the
interface reads, rather than leaving the entry to be spent by the next undo on
a count belonging to an earlier command.

What a rebuild cost SHALL be read from the history either side of it rather
than assumed, and a refused rebuild SHALL bank nothing.

#### Scenario: A rebuild is one thing to take back
- **WHEN** the user rebuilds a mesh layer's topology
- **THEN** the history offers exactly one more thing to undo than it did before

#### Scenario: One undo after a rebuild leaves the subtools standing
- **WHEN** the user strokes a form, adds a subtool, crosses into a mesh,
  rebuilds it and undoes once
- **THEN** the document holds the subtools it held before the rebuild
