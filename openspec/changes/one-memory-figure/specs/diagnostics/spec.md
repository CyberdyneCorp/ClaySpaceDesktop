## ADDED Requirements

### Requirement: The figure in use counts what the application holds to draw
The application SHALL report one figure for the memory a document is holding,
and that figure SHALL be the engine's report with the surfaces folded in, plus
the brick cache, plus what the application holds to draw the document: the
per-key copy of the surface the viewport draws from, the vertex and index
buffers at their capacity, the upload staging the graphics device may not have
finished with, and the render targets alive at the time — the window's
framebuffer, the shadow map and any capture target.

Each of those SHALL be counted where it is allocated rather than estimated from
the document: a buffer or target SHALL add its bytes when it is created and
give them back when it is dropped, so the figure cannot outlive the allocation
or miss it.

The report SHALL list the brick cache and the drawing as parts of their own,
and the drawing's four parts beside it, so a reader can see where the memory
is and not only how much. The status area, the diagnostics report and an
agent's state SHALL all read this one figure, assembled in one place.

#### Scenario: Drawing a surface moves the figure
- **WHEN** the viewport meshes and uploads a surface
- **THEN** the figure in use grows by at least what the stored geometry and the
  buffers it was uploaded into occupy

#### Scenario: A freed allocation leaves the figure
- **WHEN** a buffer is replaced by a larger one, or a capture target is thrown
  away after its frame
- **THEN** the figure no longer counts the bytes of what was dropped

#### Scenario: Staging is counted until the device is done with it
- **WHEN** geometry is written to the device
- **THEN** the written bytes are counted as staging until the device has been
  seen to finish with them, and not after

#### Scenario: The figure tracks the process
- **WHEN** the ten-times reference scene is built, meshed, uploaded and drawn,
  once the process has drawn a scene of that size before
- **THEN** the process footprint grows by no more than twice the figure in use
  reported for it

### Requirement: A footprint the ledger does not explain is logged
The application SHALL compare what the operating system charges the process
against the figure in use, off the interface thread and on an interval, and
SHALL log a footprint larger than twice the figure in use plus a stated
allowance for the process at rest. The line SHALL carry the figure's breakdown
and the amount unaccounted for.

It SHALL log once when the gap is first seen and again each time the footprint
has doubled since, and SHALL re-arm when the footprint falls back within
bounds: a leak is a figure that keeps growing, and a line a second saying the
same thing is noise.

#### Scenario: The audited gap is flagged
- **WHEN** the process footprint is 26 GB and the figure in use is 13 MB
- **THEN** the application logs the footprint, the figure and its breakdown

#### Scenario: A process at rest is not flagged
- **WHEN** a small document is open in a process whose footprint is its code,
  its driver and its interface
- **THEN** nothing is logged
