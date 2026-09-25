## MODIFIED Requirements

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

The application SHALL draw the command's requested frame before taking a
ride-along capture, including updated interface primitives. A whole-window
capture SHALL place the scene in the same viewport rectangle as the displayed
window.

An agent SHALL be able to choose a camera preset for one capture. The
application SHALL preserve the sculptor's live camera after the capture.
Remembered frames, their comparisons and forget operations SHALL be scoped to
the caller's MCP session.

#### Scenario: A command opens a panel
- **WHEN** an agent opens a panel and requests a whole-window capture with the command
- **THEN** the image shows the open panel and the scene occupies the displayed viewport rectangle

#### Scenario: A capture uses a temporary camera
- **WHEN** an agent captures with the front camera preset
- **THEN** the image uses the front view and the sculptor's camera is unchanged

#### Scenario: Callers remember frames independently
- **WHEN** two MCP sessions remember a frame under the same name
- **THEN** each session compares and forgets only its own frame
