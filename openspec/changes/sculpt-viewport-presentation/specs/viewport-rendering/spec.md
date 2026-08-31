## MODIFIED Requirements

### Requirement: Reference overlays are available and unobtrusive
The viewport SHALL offer a ground grid and a symmetry-plane indicator as toggleable overlays. Overlays SHALL render behind or beneath the sculpt in visual weight, SHALL never obscure the silhouette, and SHALL be excluded from every export.

The ground grid SHALL dissolve before it reaches its own extent, so that it
draws no boundary around the scene. The dissolve SHALL vary along each line
rather than per line, so that a line running past the form thins as it leaves
it.

The grid SHALL distinguish major lines from minor ones, so that a distance can
be counted rather than estimated.

#### Scenario: The grid draws no rectangle around the scene
- **WHEN** the ground grid is shown
- **THEN** its outermost lines have reached the viewport's ground tone, and no edge is visible where the grid ends

#### Scenario: The grid is strongest under the form
- **WHEN** the ground grid is shown
- **THEN** the lines near the origin are stronger than the same lines further out

#### Scenario: The symmetry plane does not cross the form
- **WHEN** a symmetry plane is shown with the camera inside the plane's extent
- **THEN** the indicator is the plane's outline and centre lines only, drawn at
  a fraction of the accent, with no lattice of lines across the sculpt

### Requirement: Interactive frames render at a lower quality than idle ones
The viewport SHALL carry an explicit quality state, and the application — not
the renderer — SHALL choose it from what the pointer is doing. A stroke in
progress SHALL drop occlusion sample count, disable the cavity term and
disable temporal accumulation, whatever quality profile is selected.

The state SHALL NOT change on every pointer event: it SHALL fall to the
interactive tier immediately on pointer down and rise again only after the
pointer has been still for a stated interval.

The quality profile SHALL be selectable by the user. Choosing one SHALL change
what an idle frame is drawn with and SHALL NOT change what is drawn, so it
SHALL emit no command and SHALL enter no history.

#### Scenario: A stroke does not pay for idle quality
- **WHEN** a stroke is in progress under the Presentation profile
- **THEN** the frames drawn during the stroke use the interactive occlusion
  sample count and draw no cavity term

#### Scenario: Quality does not oscillate
- **WHEN** the pointer is pressed and released repeatedly in quick succession
- **THEN** the quality state does not rise between the events, and rises only
  once the pointer has been idle for the stated interval

#### Scenario: The renderer is told, not asked
- **WHEN** the renderer draws a frame
- **THEN** it takes the quality and interaction state it was given, and reads
  no pointer or input state of its own

#### Scenario: A profile can be chosen
- **WHEN** the user chooses a viewport quality profile
- **THEN** the governor is given it, and subsequent idle frames are drawn at that profile's ceiling

#### Scenario: Choosing a profile changes no document
- **WHEN** the user chooses a viewport quality profile
- **THEN** no command is emitted and the edit history is unchanged
