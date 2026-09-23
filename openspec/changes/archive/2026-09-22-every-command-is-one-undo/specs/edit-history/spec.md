## ADDED Requirements

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
