# edit-history Specification

## Purpose
Undo and redo: what counts as one entry, what is deliberately kept out of the
history, how deep it goes, and the cost rule that says taking an edit back costs
what making it cost rather than what the whole document costs.

## Requirements

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

### Requirement: Undo costs what the edit cost
Taking a step through the history SHALL re-mesh the region that step actually
changed, and not the whole of any layer. The application SHALL take the
region from the engine, which reports it, rather than deriving one.

The region is a world region and not a layer's, so a step SHALL be re-meshed
wherever it landed, including on a subtool that is not the active one.

Where the engine reports no finite region — a non-local operation, an
unbounded primitive — the application SHALL fall back to dirtying everything
rather than guessing a box. Where it reports that nothing changed, the
application SHALL re-mesh nothing.

#### Scenario: An undo is as cheap as the dab it reverses
- **WHEN** the user undoes a brush dab
- **THEN** the surface re-meshed is the dab's own neighbourhood, not the
  layer's, and the undo costs about what the dab cost

#### Scenario: An undo on another subtool is drawn
- **WHEN** the user makes an edit on one subtool, activates another, and undoes
- **THEN** the edit is taken back *and* the subtool it was made on is
  re-meshed, rather than the active one

#### Scenario: An unbounded step still dirties everything
- **WHEN** the step undone cannot be bounded by a finite region
- **THEN** the whole layer is re-meshed rather than a guessed region

### Requirement: A gesture on a hierarchy is one undo, and it is exact
A stroke on a subdivision hierarchy SHALL be one step in the same history every
other edit goes into, however many segments drew it. Undoing it SHALL restore
the surface exactly rather than approximately, and redoing it SHALL restore
what was taken back.

Both directions SHALL tell the viewport to draw again. A hierarchy rebuilt from
a record is a different surface that may report the same revision as the one it
replaced, so the application SHALL NOT rely on the engine's counters alone to
notice that it moved.

#### Scenario: A drag is one step
- **WHEN** the user drags a brush across a hierarchy and releases
- **THEN** one undo takes the whole gesture back

#### Scenario: The form comes back exactly
- **WHEN** a gesture on a hierarchy is undone
- **THEN** every vertex is where it was before the gesture, and the viewport
  shows it

#### Scenario: A redo is seen
- **WHEN** an undone gesture on a hierarchy is redone
- **THEN** the surface is what it was after the gesture, and the viewport shows
  it rather than continuing to draw what it last uploaded

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

### Requirement: An edit to the mask is one step of the history the interface reads
The history a user steps through is the one the application keeps, which counts
**actions** and remembers how many engine entries each action spent. Every edit
to a mask — inverting, clearing, expanding, contracting, smoothing, the bounded
complement, a region drawn round the form, and pulling the frozen patch off as
a wall — SHALL bank exactly one action on that history, whatever it cost
underneath.

What an edit cost SHALL be read from the history either side of it rather than
assumed to be one entry, and an edit that wrote nothing SHALL bank nothing.

Every path that writes to a mask SHALL bank through one place, so that an
operation added later cannot arrive without an entry.

#### Scenario: Each mask operation is one thing to take back
- **WHEN** the user applies any operation from the mask menu, draws a region or
  extrudes the frozen patch
- **THEN** the history offers exactly one more thing to undo than it did before

#### Scenario: An undo after a mask edit takes back the mask edit
- **WHEN** the user removes a subtool, paints a mask, strokes the clay, clears
  the mask and undoes once
- **THEN** the frozen region comes back, the clay the stroke moved is where the
  stroke left it, and the subtools are the ones the document had

#### Scenario: A redo puts the operation back
- **WHEN** the user takes a mask operation back and redoes
- **THEN** the mask is as the operation left it

#### Scenario: An operation that wrote nothing is not something to take back
- **WHEN** the user clears a mask that freezes nothing
- **THEN** the history offers exactly what it did before, and the user is told
  there was nothing to clear rather than being refused

### Requirement: A history step refreshes what is drawn of the mask
A mask is drawn but is not geometry, so nothing the surface reports tells the
viewport that a frozen region changed; every site that writes to a mask says so
beside the write. The engine's history writes to a mask through a snapshot of
its own, past those sites, so a step through the history SHALL itself say that
the frozen region may have moved.

#### Scenario: An undone mask operation is redrawn
- **WHEN** the user applies a mask operation and undoes it
- **THEN** the surface is drawn with the mask the undo restored rather than the
  one the operation left

### Requirement: Cancelling a gesture spends only what that gesture wrote
Cancelling the gesture in progress SHALL take the document back to where it
stood when the gesture opened, and SHALL NOT step past that point. How far back
that is SHALL be measured from the document rather than counted from the
segments the gesture was sent in: a representation may record an entry per
segment or bank the whole gesture as one record, and a cancel that assumed the
first destroyed work on the second.

Nothing a cancel takes back SHALL be offered as something to put forward again:
a cancelled gesture is not an action the user can ask for back.

#### Scenario: A cancel reaches no further than its own gesture
- **WHEN** the user commits gestures, begins another and cancels it
- **THEN** the history offers exactly the committed gestures to undo, and every
  one of them is still on the document

#### Scenario: A cancel is not something to redo
- **WHEN** the user cancels a gesture
- **THEN** the history offers nothing more to put forward than it did before the
  gesture began

### Requirement: A cancel with no gesture open changes nothing and says so
A release arrives whether or not the press that should have preceded it opened
anything, so a cancel with nothing open SHALL NOT be a refusal — a caller may
safely repeat one. It SHALL change nothing: no history is stepped, and the
model is not told that a gesture ended.

It SHALL be reported as a cancel that had nothing to cancel, rather than
leaving the last edit that did happen standing as what the application last
did.

#### Scenario: Cancelling with nothing open costs nothing
- **WHEN** the user cancels with no gesture open, twice
- **THEN** the document is untouched, the history offers exactly what it did
  before, and the application reports a cancel that changed nothing

### Requirement: Every structural and deformation command is one step of the history the interface reads
The history a user steps through is the one the application keeps, which counts
**actions** and remembers how many engine entries each action spent. Applying a
deformation cage, running a deformer, a manipulator drag on an object, a
subtool or the whole scene, inserting a shape, copying a subtool, changing what
a placed object is or how it combines, removing one, adding a subtool, removing
one, renaming one, reordering the stack, consolidating a subtool, rebuilding a
mesh subtool's topology and resolving a boolean SHALL each bank exactly one
action on that history, whatever each cost underneath.

What a command cost SHALL be read from the history either side of it rather
than assumed to be one entry, and a command that wrote nothing SHALL bank
nothing.

Each SHALL bank where the change is made rather than in the shell, so that a
command added later cannot arrive without an entry.

Changing which subtool is active, what is drawn, which subtool is shown alone,
and which level or recorded pass of a subdivision hierarchy is selected SHALL
NOT bank anything: those are ways of looking at the scene and at the stack, and
a user whose next undo took back a click on a row would have to choose between
navigating and working.

#### Scenario: Each command is one thing to take back
- **WHEN** the user applies any of these commands
- **THEN** the history offers exactly one more thing to undo than it did before

#### Scenario: An undo after a bend leaves the subtools alone
- **WHEN** the user adds a subtool, inserts a shape into it, bends it through a
  cage and undoes once
- **THEN** the form returns to where the bend found it, and every subtool the
  document held is still there

#### Scenario: An undo after adding a subtool removes only that subtool
- **WHEN** the user adds a subtool and undoes
- **THEN** the document holds exactly the subtools it held before

#### Scenario: A mixed session undoes one command at a time
- **WHEN** the user runs a session mixing subtools, insertions and bends, and
  undoes back to the start
- **THEN** each step lands on the document as it stood before the command it
  was meant for, no undo removes a subtool the undone command did not create,
  and the history ends empty

#### Scenario: A refused command is not something to take back
- **WHEN** a command is refused and the document is left as it was
- **THEN** the history offers exactly what it did before

### Requirement: A manipulator drag is one undo for the whole gesture
A drag sets a transform on every frame while the pointer is down, and the
engine groups those frames. The application SHALL bank one action for the
gesture, measured from the history depth read at the press, and SHALL NOT bank
anything while the gesture is open.

Measuring rather than counting the frames is required by both ends of the
gesture: the frames coalesce into one entry underneath, and a frame that
overran the budget leaves the document until the release, so the number of
calls made is not the number of entries written.

#### Scenario: A drag is one undo, not one per sample
- **WHEN** the user drags a manipulator handle across many frames and releases
- **THEN** the history offers one more thing to undo than it did before, and
  one undo puts the target back where the press found it

#### Scenario: A release with no press before it costs nothing
- **WHEN** a release arrives with no drag open
- **THEN** the history offers exactly what it did before

### Requirement: What a gesture cost is measured from the depth it opened at
A gesture's cost SHALL be the distance between the document's history depth
when the gesture opened and its depth when the gesture ended, whichever way it
ended. It SHALL NOT be counted from the segments the gesture was sent in: a
representation may record an entry per segment or bank the whole gesture as a
single record, and the count that is right for one is wrong for the other by
however many segments drew it.

A gesture that wrote nothing SHALL bank nothing, and whether it wrote anything
SHALL be decided from what it put into the document rather than from where it
started — a dab aimed off the surface can still deposit material, and that is
an edit.

#### Scenario: A committed gesture on carried geometry is one record
- **WHEN** the user draws a gesture in several segments on a subtool that banks
  the whole gesture as one record, and undoes once
- **THEN** the gesture is taken back and nothing under it is, including the
  subtool it was drawn on

#### Scenario: A committed gesture that records per segment still undoes whole
- **WHEN** the user draws a gesture in several segments on a subtool that
  records an entry per segment, and undoes once
- **THEN** every entry the gesture made is taken back, and nothing under them is

#### Scenario: A stroke that wrote nothing is not something to take back
- **WHEN** a stroke leaves the document exactly as it found it
- **THEN** the history offers exactly what it did before

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
