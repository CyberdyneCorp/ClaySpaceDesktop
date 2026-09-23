## ADDED Requirements

### Requirement: The history names the next step in each direction
The state an agent reads SHALL name what the **next undo would revert** and
what the **next redo would restore**, and SHALL report a depth for each
direction.

It SHALL NOT report the last action that happened in place of either. The two
differ exactly where it matters: after an undo the last action is the undo, and
after a cancelled gesture it is the tool whose work the cancel already took
back. A status area can live with the difference because a person watched it
happen; an agent reading state cannot.

Where there is nothing to undo or nothing to redo, the answer SHALL say so
rather than naming a step nothing would take. Where a step exists and nothing
named it, the depth SHALL still be reported: a count without a name is a
different answer from no step at all.

The same question asked in the answer to an applied command SHALL be answered
the same way, so a command that banked nothing names the command before it
rather than itself.

#### Scenario: The label is the next step, not the last one
- **WHEN** an agent applies a stroke, undoes it, and reads the history
- **THEN** the undo side names nothing left to take back, and the redo side
  names the stroke it would put back — not the undo that just happened

#### Scenario: A cancelled gesture names nothing of its own
- **WHEN** an agent opens a gesture, cancels it, and reads the history
- **THEN** the next undo is named as the action *before* the cancelled
  gesture, which is what an undo would actually revert

#### Scenario: Both directions carry a depth
- **WHEN** an agent has undone several actions
- **THEN** the history reports how many actions can be undone and how many can
  be redone

### Requirement: Every part of the session a command can change is readable
An agent SHALL be able to read, as values: the brush panel's settings beside
the tool's size and strength; how a stroke and how a placed form each combine
with what is already there; the mask's coverage, frozen cells, step count and
gesture; the cage and what is selected in it; the deform settings; the forms
placed in the document, each named by an identifier the selection can be
compared against; which layer is being shown alone; a grid's cells and its
recorded passes; a hierarchy's levels, write domain and passes; what the last
rebuild, retopology and crossing came to; how the viewport is presented; the
reference images plane by plane; and what an import or an export would be
given.

Every section an agent may ask for by name SHALL be a section the report
answers under that same name, and the set of names SHALL be stated once —
so that a section the reader knows about, a section the tool surface
advertises and a section the answer carries cannot be three different sets.

A count SHALL mean one thing across the representations it is reported on. In
particular the count of placed forms in a layer SHALL be the forms placed in
it, and a grid's recorded passes SHALL be reported under their own name.

Where an operation has not been run, its outcome SHALL be absent rather than
reported as zero: "it has not been done" and "it was done and changed nothing"
are different answers, and an agent verifying an operation branches on which.

#### Scenario: A command's effect is observable without a picture
- **WHEN** an agent sets a brush's flow, a combine mode, a mask's step count,
  a deform's angle or an export's mesher, and reads state back
- **THEN** the value it set is in the answer

#### Scenario: A layer's forms and a grid's passes are different counts
- **WHEN** an agent reads a grid layer that holds placed forms and recorded
  passes
- **THEN** the forms are counted as forms and the passes are reported under
  their own name, neither standing for the other

#### Scenario: A soloed scene says so
- **WHEN** an agent shows one layer alone and reads the scene
- **THEN** the answer names the layer being shown alone, rather than leaving a
  solo indistinguishable from layers hidden by hand

#### Scenario: A rebuild that has not run reports nothing
- **WHEN** an agent reads the outcomes of a session in which nothing has been
  rebuilt
- **THEN** no rebuild outcome is reported, rather than an outcome of zero
  triangles

### Requirement: A selection that no longer exists is not reported
Where the node a selection names is no longer in the document, the report SHALL
carry no selection rather than the identifier of a node that is gone.

#### Scenario: An undo that removed the form clears the selection
- **WHEN** an agent places a form, selects it, undoes the placement and reads
  the scene
- **THEN** nothing is reported as selected

### Requirement: Two mirrors are reported as two switches
Where a rig is being edited, the state SHALL report the rig's own mirror
alongside the brush's symmetry axes, and SHALL NOT fold one into the other.
They decide different things: the axes decide what a brush stamps, and the rig
mirror decides whether a node grown on the rig gets a partner.

Where nothing is being rigged, the rig's mirror SHALL be absent rather than
reported as off.

#### Scenario: A rig mirroring its spheres says so
- **WHEN** an agent reads the tool state while editing a rig whose mirror is on
  and whose brush symmetry is off
- **THEN** the answer reports no symmetry axes and reports the rig's mirror as
  on

### Requirement: A memory figure says which figure it is
Where the application shows more than one memory figure, the state SHALL report
each under its own name rather than reporting one as though it were the other.

The figure the status area shows and the figure the document's ledger reports
count different things, and an agent comparing the two against each other
without being told which is which reads the difference as a defect.

#### Scenario: Both figures are named
- **WHEN** an agent reads memory
- **THEN** the document's ledger and the figure the status area shows are both
  in the answer, each under its own name, alongside the budget
