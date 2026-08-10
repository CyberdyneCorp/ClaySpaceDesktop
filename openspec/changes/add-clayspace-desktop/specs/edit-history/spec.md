## ADDED Requirements

### Requirement: Every document mutation is undoable
Every operation that changes document state SHALL be undoable and redoable through the engine's undo vocabulary. An operation that cannot be expressed as an undoable command SHALL NOT be offered.

#### Scenario: Undo restores the prior state exactly
- **WHEN** any document-modifying operation is performed and then undone
- **THEN** the document is in the state it held before that operation, and the viewport reflects it

#### Scenario: Redo restores the undone state
- **WHEN** an operation is undone and then redone
- **THEN** the document matches the state after the original operation

### Requirement: A stroke is one history entry
A continuous stroke SHALL coalesce into a single undo entry regardless of how many stamps or samples it produced. Undoing a stroke SHALL remove the whole stroke, not its last stamp.

#### Scenario: A long stroke undoes as one
- **WHEN** the user draws a stroke producing many stamps and presses undo once
- **THEN** the entire stroke is removed

#### Scenario: A drag coalesces rather than accumulating
- **WHEN** the user drags the Move brush continuously and then undoes
- **THEN** the whole drag is removed in one step, matching the engine's drag coalescing

### Requirement: Compound operations are grouped
Operations that produce several engine edits — a symmetric edit, a multi-layer operation, an imported mesh with its layer — SHALL be wrapped in an undo group and SHALL undo as a single step.

#### Scenario: A symmetric edit undoes as one step
- **WHEN** an edit is applied with symmetry active and the user presses undo once
- **THEN** both the original and the mirrored edit are removed

### Requirement: History is presented and navigable
The application SHALL present the undo history as a list of named entries in order, indicate the current position, and allow the user to move to any entry in the list.

#### Scenario: Jumping back several steps
- **WHEN** the user selects an entry several steps back in the history
- **THEN** the document reaches the state at that entry, equivalent to undoing each intervening step

#### Scenario: Entries are named for what they did
- **WHEN** the history is displayed after a mix of operations
- **THEN** each entry names the operation it represents rather than showing a generic label

### Requirement: A new edit after undo replaces the redo branch
Performing a new edit while positioned before the end of the history SHALL discard the redo entries beyond the current position, and the interface SHALL make that outcome evident before it is irreversible.

#### Scenario: Redo entries are discarded on a new edit
- **WHEN** the user undoes twice and then makes a new edit
- **THEN** the two undone entries are no longer redoable, and the new edit is the newest entry

### Requirement: Operations that change nothing add no history
An operation the engine reports as having changed nothing SHALL NOT create a history entry, and SHALL NOT mark the document modified.

#### Scenario: A no-op leaves history untouched
- **WHEN** a verb runs over a region it does not change
- **THEN** the history is unchanged and the document's modified state is unchanged

### Requirement: History has a bounded, configurable depth
The application SHALL bound the undo history by a configurable number of entries or memory budget, discarding the oldest entries when the bound is reached, and SHALL show the configured bound to the user.

#### Scenario: Oldest entries are discarded at the bound
- **WHEN** the number of entries exceeds the configured depth
- **THEN** the oldest entries are discarded and the most recent entries remain undoable

### Requirement: Non-document state is excluded from history
Camera movement, view preset changes, material selection, panel layout and selection changes SHALL NOT create undo entries.

#### Scenario: Orbiting does not fill the history
- **WHEN** the user orbits the camera, switches view preset and changes MatCap
- **THEN** the undo history is unchanged
