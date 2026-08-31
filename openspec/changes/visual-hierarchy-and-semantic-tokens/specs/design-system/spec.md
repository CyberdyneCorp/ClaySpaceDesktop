## MODIFIED Requirements

### Requirement: The interface uses the defined palette
The interface SHALL be built from a defined token palette. The palette SHALL
name four surfaces, ordered from darkest to lightest: `#1B1E22` as the
sculpting viewport's ground, `#23262B` as the application shell's ground,
`#2E3238` as a panel sitting on the shell, and `#3A3E45` as a raised surface.
It SHALL further name `#C9C4BD` as the primary foreground and `#D9744A` as the
sole accent. Colors SHALL be referenced through named tokens, and no component
SHALL introduce a literal color outside the token set.

The viewport's ground SHALL be darker than the application's, so that the
sculpt is separated from the chrome around it by tone rather than by an
outline.

Grid lines are drawn on the viewport's ground rather than on the shell's, and
their tones SHALL be defined relative to it: each SHALL be lighter than the
viewport ground and the axis lines SHALL be lighter than the minor ones, while
both SHALL stay far below the foreground so the grid never competes with the
form standing on it.

#### Scenario: No literal colors in components
- **WHEN** the interface source is inspected
- **THEN** every color originates from a named token, and the token set defines the palette values plus derived states

#### Scenario: Panels and ground are distinguished by tone, not by border
- **WHEN** a panel sits against the application ground
- **THEN** it is distinguished by its surface tone, without a drawn outline competing for attention

#### Scenario: The four surfaces are ordered
- **WHEN** the viewport ground, the shell ground, a panel and a raised surface are compared by relative luminance
- **THEN** each is lighter than the one before it, so that a panel reads as sitting on the shell and the shell as surrounding the viewport

#### Scenario: The viewport is separated from the chrome
- **WHEN** the viewport is displayed beside the panel regions
- **THEN** the viewport's ground is darker than the shell's, and the boundary is carried by that difference rather than by a drawn edge

#### Scenario: The grid keeps its distance from its own ground
- **WHEN** the viewport ground is changed
- **THEN** the grid's tones are defined against it, so the grid does not become more or less prominent as a side effect

### Requirement: The accent marks active state
`#D9744A` SHALL indicate active state — the active brush, the active tool
state, the layer being sculpted, an engaged toggle — and nothing else. It SHALL
NOT be used for panel chrome, headings, borders, hover states, or decoration.

The accent SHALL be applied at the scale of a rail, a mark, a ring or a label.
It SHALL NOT fill a panel, a row or a card: an active row is a raised surface
carrying an accent rail, never an accent-filled rectangle.

Active state SHALL NOT be carried by the accent alone. Every accented state
SHALL also be distinguished by surface tone, text weight or geometry, so that
it survives a color-vision deficiency and a high-contrast theme.

#### Scenario: The accent stays small
- **WHEN** any active state is indicated
- **THEN** the accent occupies a rail, a mark, a ring or a label, and no filled panel, row or card is drawn in it

#### Scenario: One accent on screen
- **WHEN** the application is displaying a document with a brush selected
- **THEN** the accent appears on the active brush, on the active layer's rail, and on tool state, and nowhere else

#### Scenario: The active layer is identifiable at a glance
- **WHEN** a layer stack holds several layers and one is active
- **THEN** the active row carries an accent rail at its leading edge, a raised surface, and its name in primary rather than secondary text

#### Scenario: Selection survives the hue being removed
- **WHEN** the active layer and the active brush are rendered with the accent hue removed
- **THEN** both are still identifiable, by surface tone and text weight for the layer and by tone, ring and card for the brush

### Requirement: Controls are quiet until addressed
Controls SHALL present at low contrast when inactive and gain contrast on
hover, focus and while being adjusted. The interface SHALL NOT compete with the
sculpt for attention while the user is not addressing it.

A control drawn by hand rather than taken from the toolkit SHALL keep the
keyboard behaviour the toolkit's equivalent had. A control that takes keyboard
focus and does nothing once it has it is worse than one that cannot be reached
at all, because nothing on screen says which it is.

#### Scenario: Hover raises contrast
- **WHEN** the pointer enters a control
- **THEN** that control's contrast increases and no other control changes

#### Scenario: Sliders read as a traversed range and a position
- **WHEN** a slider is displayed
- **THEN** it is drawn as a track in the ground's tone, with the range from the track's start to the current position filled in the accent, a position marker, and its numeric value set monospaced beside it

#### Scenario: The fill states the position and does not decorate
- **WHEN** a slider's value is at the start of its range
- **THEN** no fill is drawn, so the fill is only ever the distance travelled and never a bar that brightens the control

#### Scenario: A slider is quiet until the pointer reaches it
- **WHEN** a slider is not hovered and not being dragged
- **THEN** its position marker is drawn in a resting tone and gains contrast only when the pointer enters it or a drag begins

#### Scenario: A focused slider answers the arrow keys
- **WHEN** a slider holds keyboard focus and an arrow key along its axis is pressed
- **THEN** the value moves by at least one unit of the slider's own displayed precision, and focus stays on the slider rather than moving to the next control

#### Scenario: A press is visible in the readout
- **WHEN** a slider showing whole numbers is adjusted by one arrow-key press
- **THEN** the number beside it changes, rather than moving by a fraction that rounds back to what it was

### Requirement: Every brush swatch carries a mark of its own
Each brush swatch on the shelf SHALL carry a drawn mark depicting the brush's
effect on a surface, distinct from every other brush's mark, drawn from the
same line-based set at one stroke weight and contained within the swatch.
Hovering a swatch SHALL show the brush's name and one sentence saying what it
does, in the interface's language. The mark SHALL NOT use the accent.

The swatch SHALL be drawn at the size the scale reserves for a brush swatch,
which is large enough for the mark inside it to be read without hovering. A
swatch SHALL NOT be sized from a token named for another control.

#### Scenario: Two brushes are told apart without their labels
- **WHEN** the shelf is displayed with the labels covered
- **THEN** no two swatches show the same mark

#### Scenario: A swatch explains itself on hover
- **WHEN** the pointer rests on a swatch
- **THEN** the brush's name and a one-sentence description appear, translated to the interface's locale

#### Scenario: The mark is legible without hovering
- **WHEN** the shelf is displayed
- **THEN** each swatch is drawn at the scale's brush-swatch size, and the mark inside it is readable at that size

### Requirement: Spacing and sizing come from a defined scale
Spacing, control heights, icon sizes and corner radii SHALL be drawn from a
defined scale rather than chosen per component, so that panels align across the
application. Each size in the scale SHALL be used by the control it is named
for, and a control SHALL NOT borrow a size named for another.

#### Scenario: Panels align across regions
- **WHEN** the left and right panel regions are displayed together
- **THEN** their section headings, row heights and internal padding align on the same scale

#### Scenario: Sections are separated by the section step
- **WHEN** two sections of a panel are displayed one above the other
- **THEN** the space between them is the scale's section step, which is larger than the step between groups within a section

#### Scenario: No size is named for a control that does not use it
- **WHEN** the scale is inspected against the components that draw from it
- **THEN** every named size is used by the control its name describes
