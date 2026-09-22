## ADDED Requirements

### Requirement: The history orders itself by a sequence, not by a depth
The document SHALL keep a monotonic sequence of its own and stamp every
undoable thing with it: each entry the engine records, each gesture on carried
geometry, each crossing, and each batch of visibility commands the document
issued for its own reasons. The sequence SHALL NOT be decremented — a step
through the history moves a stamp between the undo and redo sides rather than
handing the number back.

No ordering decision SHALL be made by comparing the engine's undo depth, which
is a stack size: it repeats, it falls when a command coalesces, and it stops
rising once the engine's history budget evicts as many entries as it gains.

Which history a step moves SHALL be decided in one place, from the stamps. An
undo SHALL move the thing with the greatest stamp; a redo SHALL move the thing
with the smallest stamp among those an undo took back.

#### Scenario: A gesture and a way of looking at the scene are ordered
- **WHEN** the user makes a stroke on carried geometry and then shows one
  subtool alone, and undoes once
- **THEN** the solo is stepped over and the stroke is taken back, and when the
  solo was engaged *before* the stroke the stroke is taken back with the solo
  left standing

#### Scenario: One undo reverts one command
- **WHEN** the user runs a mixed session of strokes, subtools and crossings and
  undoes through it
- **THEN** each step lands on the document as it stood before the command it
  was meant for, and no layer leaves the scene that the command did not add

#### Scenario: A redo reaches the document the run ended at
- **WHEN** the user applies a run of commands, takes every one of them back, and
  puts every one of them forward again
- **THEN** the layer set, what each layer holds and the drawn geometry are what
  they were when the run ended

### Requirement: A new edit ends the redo line, including on the document's own stacks
When an entry lands in the engine's history the document SHALL drop every
record it is holding on the redo side — gestures on carried geometry,
crossings, and visibility batches alike — because the future they describe has
been built over, exactly as the engine discards its own redo stack.

#### Scenario: A gesture taken back before a new edit is not put back after it
- **WHEN** the user takes a stroke on carried geometry back, makes an unrelated
  edit, takes *that* edit back, and redoes
- **THEN** the redo puts the unrelated edit back, and the stroke stays where
  the user left it

### Requirement: An evicted engine entry takes its record with it
A crossing and a visibility batch are named by the engine entries they left
behind. Where the engine drops such an entry — evicting its oldest to stay
inside a history budget — the document SHALL drop the record that named it,
rather than leaving it to be matched against an entry belonging to something
else.

A gesture on carried geometry is not an engine entry and SHALL survive an
eviction; what bounds that stack is its own byte budget.

#### Scenario: A crossing whose entry has gone is no longer on the stack
- **WHEN** the engine evicts the entry a recorded crossing named
- **THEN** the crossing is no longer something an undo can reach, and the next
  undo moves the newest entry the engine still holds

#### Scenario: An eviction leaves the carried gestures standing
- **WHEN** the engine evicts entries from its own history
- **THEN** the gestures on carried geometry, which cost the engine no entry,
  are still there to be taken back

### Requirement: The reported history counts every command that changed the document
The depth the interface is shown SHALL rise by one for every command that
changed the document, including the ones the engine records nothing for, and
SHALL NOT rise for a way of looking at the scene. A crossing SHALL count as the
one thing the user asked for however many entries it left behind.

#### Scenario: A stroke on carried geometry is something to take back
- **WHEN** the user strokes a carried mesh, which the engine records no entry
  for
- **THEN** the history offers one more thing to undo than it did before

#### Scenario: Showing one subtool alone is not
- **WHEN** the user shows one subtool alone
- **THEN** the history offers exactly what it did before
