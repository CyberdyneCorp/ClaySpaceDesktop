## Purpose

Lets an agent driving a live session see and measure it — the viewport as an
image that provably shows the state it is claimed to show, the scene and tool
state as values, and timing and memory figures taken from the running session
rather than from a fresh process.

## ADDED Requirements

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
