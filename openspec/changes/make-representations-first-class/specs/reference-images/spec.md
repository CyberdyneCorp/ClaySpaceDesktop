## Purpose

Reference images: what an artist pins to the wall beside the monitor, put where
it belongs — on the plane a view preset looks down, behind the form so the
silhouette can be read against it. A guide to sculpt from, and never part of
what is being made.

## ADDED Requirements

### Requirement: A reference image can be placed on each plane
The application SHALL let the user load a PNG or a JPEG onto each of the front,
side and top planes independently, and SHALL keep each plane's picture and
placement separate from the others.

#### Scenario: One plane at a time
- **WHEN** the user loads an image onto the front plane
- **THEN** that image appears on the front plane and the side and top planes are
  unchanged

#### Scenario: A file that cannot be read is refused by name
- **WHEN** the user chooses a file that is neither a PNG nor a JPEG, or a
  picture too small or too large to be used
- **THEN** the application states which of those it is and places nothing

#### Scenario: A photograph taken sideways is turned the right way up
- **WHEN** a JPEG carries an EXIF orientation tag
- **THEN** the reference is drawn the right way up, with its sides swapped
  where the tag is a quarter turn

### Requirement: A reference is placed, sized and faded by the sculptor
The application SHALL offer, for each plane holding a picture, its visibility,
its opacity, its height, its offset within its own plane, and how far behind the
origin it sits. Width SHALL follow from the image's own proportions, so a
reference is never squashed.

#### Scenario: The opacity reaches the screen
- **WHEN** the user lowers a reference's opacity
- **THEN** the drawn image fades, and at zero it is not drawn at all

#### Scenario: The proportions are kept
- **WHEN** an image twice as wide as it is tall is placed
- **THEN** the drawn quad is twice as wide as it is tall, whichever plane it is
  on

#### Scenario: An empty plane offers no placement
- **WHEN** a plane holds no picture
- **THEN** the panel offers to load one and draws no placement controls

### Requirement: The clay is always in front of the reference
The application SHALL draw every reference behind the sculpted form, from any
camera angle, including when the camera is on the far side of the reference's
own plane.

#### Scenario: Seen from behind
- **WHEN** the camera orbits past a reference's plane so the plane lies between
  the camera and the form
- **THEN** the form is still drawn over the reference

### Requirement: References are session state and not document content
The application SHALL keep reference images out of the document, and SHALL
remember each plane's file path and placement with the session instead. A
remembered file that can no longer be read SHALL be dropped quietly.

#### Scenario: A reference does not modify the document
- **WHEN** the user loads, moves, fades or clears a reference
- **THEN** the document is not marked as modified

#### Scenario: The placement survives a restart
- **WHEN** the user places a reference and reopens the application
- **THEN** the same file is on the same plane with the same opacity, height and
  offsets

#### Scenario: A file that has moved is dropped
- **WHEN** a remembered reference's file no longer exists at its path
- **THEN** the plane opens empty rather than showing an error at startup

### Requirement: The sculpted surface can be made translucent
The application SHALL offer a control over how opaque the sculpted surface is
drawn, so that a reference behind it can be seen through the clay. The surface
SHALL NOT be reducible to fully transparent.

#### Scenario: The reference shows through the clay
- **WHEN** the user lowers the model opacity with a reference behind the form
- **THEN** the reference is visible through the form, and the form is still
  distinguishable from no form at all

#### Scenario: A solid model hides what is behind it
- **WHEN** the model opacity is left at solid
- **THEN** the reference is not visible through the form

#### Scenario: A deformation cage still imposes its own ceiling
- **WHEN** a deformation cage is raised while the model opacity is solid
- **THEN** the surface is drawn through, as it is without the control

#### Scenario: A cage does not overrule a fainter choice
- **WHEN** a deformation cage is raised while the model opacity is already
  fainter than the cage's own ceiling
- **THEN** the surface stays at the fainter setting
