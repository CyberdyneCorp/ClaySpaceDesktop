# agent-observation Specification

## Purpose
Lets an agent driving a live session see and measure it — the viewport as an
image that provably shows the state it is claimed to show, the scene and tool
state as values, and timing and memory figures taken from the running session
rather than from a fresh process.

## Requirements

### Requirement: The viewport is readable as an image
An agent SHALL be able to ask for the current viewport and receive it as an
image, in a form a client can display, together with the pixel dimensions it
was rendered at.

The image SHALL be produced by the renderer that draws the window, with the
same camera, the same shading, the same overlays and the same quality settings
that are in force. A capture that is drawn by a second path is a capture that
can disagree with what the sculptor sees, which is the one thing it exists to
rule out.

An agent SHALL be able to ask for a size other than the window's, so that a
frame can be read cheaply or examined closely, and the answer SHALL say what
size it was actually rendered at.

An agent SHALL be able to ask for the frame after a change in the same answer
as the change, so that acting and seeing the result cost one exchange.

An agent SHALL be able to ask for the whole window rather than the viewport
alone — the panels, the options bar, the scene tree and the status area as they
are drawn. A defect in what a panel says is a defect an agent cannot see in a
picture of the surface.

#### Scenario: The frame is what the window shows
- **WHEN** an agent captures the viewport
- **THEN** it receives an image of the current document, camera and overlays,
  drawn by the same renderer as the window, with its dimensions stated

#### Scenario: Acting and seeing in one exchange
- **WHEN** an agent applies a stroke and asks for the frame with it
- **THEN** the answer carries both the outcome of the stroke and the frame
  after it

#### Scenario: The interface can be seen too
- **WHEN** an agent asks for the whole window
- **THEN** it receives an image carrying the panels and bars as drawn, not the
  viewport alone

#### Scenario: A smaller frame is honoured and named
- **WHEN** an agent asks for a capture at a size other than the window's
- **THEN** the image is at that size and the answer says so

### Requirement: A capture shows the state it is claimed to show
A capture returned with a change SHALL show the document *after* that change
has reached the surface — after the re-mesh the change dirtied, not before it.

Where work is still outstanding when a capture is taken, the answer SHALL say
so and name what is outstanding, rather than returning a frame that is
mid-flight as though it were settled. An agent that reads a half-meshed surface
as a defect is an agent that files one.

#### Scenario: A stroke's frame shows the stroke
- **WHEN** an agent applies a stroke and captures with it
- **THEN** the surface in the image carries the stroke, with the dirty region
  re-meshed

#### Scenario: A frame taken mid-flight says so
- **WHEN** a capture is taken while meshing, baking or import is still running
- **THEN** the answer names what is outstanding alongside the image

### Requirement: The application can be asked to be quiet
An agent SHALL be able to wait for the session to reach a settled state — no
pending re-mesh, no running job, no queued maintenance — with a bound on how
long it will wait.

Where the bound is reached, the answer SHALL name what is still running rather
than reporting only that time ran out.

#### Scenario: Waiting for quiet
- **WHEN** an agent asks the session to settle after an import
- **THEN** the answer returns once the import, its meshing and its maintenance
  are done

#### Scenario: A bound that is reached names the work
- **WHEN** the wait's bound is reached with work still running
- **THEN** the answer names what is running and how far along it is

### Requirement: Session state is readable without changing it
An agent SHALL be able to read the state a sculptor can see: the document and
whether it is modified, the scene tree with its subtools, layers, their
representations, visibility and placement, the active selection, the active
tool and its settings, the mask's presence and coverage, the camera, the edit
history's depth and what its next undo would undo, and what jobs are running.

Reading SHALL NOT change anything. In particular it SHALL NOT mark ViewModel
state as changed and SHALL NOT cause a redraw, so that an agent polling the
session cannot be the reason an idle application never sleeps.

The values read SHALL be the same values the interface draws from, so that a
figure an agent reports and a figure a person reads cannot disagree.

#### Scenario: The tree an agent reads is the tree a person sees
- **WHEN** an agent reads the scene tree
- **THEN** it holds the same subtools, layers, representations and visibility
  as the interface's scene panel

#### Scenario: Reading does not wake the application
- **WHEN** an agent reads state repeatedly against an application receiving no
  input
- **THEN** no ViewModel reports a change and no redraw is scheduled for those
  reads

### Requirement: The live session can be measured
An agent SHALL be able to read what the session is costing: frame timings, the
operations that held the interface thread longer than a frame and how often,
the memory in use against the budget and which part of the document holds it,
the active backend and every operation that fell back to another.

An agent SHALL be able to run an operation under measurement and receive the
wall time it took and whether it stalled a frame.

Every measurement SHALL carry the conditions it was taken under — the backend,
the platform, and the fact that it came from a live session rather than from
the benchmark harness — so that a figure is not silently compared against a
baseline recorded elsewhere on a quiet machine.

A figure measured this way SHALL NOT be written into a benchmark baseline. The
baseline is what future runs are judged against and it is recorded by the
harness under stated conditions; a number taken from a session with a window
open is evidence, not a baseline.

#### Scenario: An operation is timed where the defect is
- **WHEN** an agent runs an operation under measurement in the open session
- **THEN** it receives the wall time, whether a frame was stalled, and the
  conditions the figure was taken under

#### Scenario: A live figure is not a baseline
- **WHEN** a figure measured from a live session is reported
- **THEN** it is marked as such, and no baseline file is written from it

#### Scenario: A fallback is visible
- **WHEN** an operation ran on a backend other than the active one
- **THEN** the agent can read which operation, and which backend declined it

### Requirement: A difference between two captures is read against the render floor
Where the server offers a comparison between two captures, it SHALL report the
difference against the difference two renders of the same subject already
produce on this machine, and SHALL report that floor alongside the figure.

The floor is not the same everywhere — it is zero on Linux and it is not on
macOS — and a comparison that does not carry it is a comparison an agent will
read the rasteriser through.

Where a comparison follows a re-mesh, the floor SHALL be measured through the
same path, because a re-mesh can return the same surface with its vertices in a
different order and move a rasterised edge.

#### Scenario: A difference carries its floor
- **WHEN** an agent compares two captures
- **THEN** the answer reports the pixels differing past the measured floor, and
  the floor itself

#### Scenario: A comparison after a re-mesh
- **WHEN** the two captures are separated by a re-mesh
- **THEN** the floor reported is one measured through a re-mesh, not through
  two draws of one buffer

### Requirement: The mask an agent reads is what is frozen
The mask an agent is shown SHALL answer whether anything is frozen on the
active subtool, which is the question a caller about to make a stroke is
asking. It SHALL NOT answer whether the layer carries a mask field: a document
has no verb for detaching one, so an emptied mask stays attached and the two
answers differ exactly after a clear — where an agent was told a region was
still protected that protects nothing.

#### Scenario: A cleared mask reads as nothing frozen
- **WHEN** an agent clears the mask and reads the session state
- **THEN** the mask reads as not present

#### Scenario: A painted mask reads as frozen
- **WHEN** an agent paints a mask and reads the session state
- **THEN** the mask reads as present

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
