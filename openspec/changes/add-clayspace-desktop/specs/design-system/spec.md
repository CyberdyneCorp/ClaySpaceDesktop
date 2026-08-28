## ADDED Requirements

### Requirement: The interface uses the defined palette
The interface SHALL be built from a defined token palette: `#23262B` as the primary ground, `#3A3E45` as the raised surface and separator tone, `#C9C4BD` as the primary foreground, and `#D9744A` as the sole accent. Colors SHALL be referenced through named tokens, and no component SHALL introduce a literal color outside the token set.

#### Scenario: No literal colors in components
- **WHEN** the interface source is inspected
- **THEN** every color originates from a named token, and the token set defines the four palette values plus derived states

#### Scenario: Panels and ground are distinguished by tone, not by border
- **WHEN** a panel sits against the application ground
- **THEN** it is distinguished by its surface tone, without a drawn outline competing for attention

### Requirement: The accent marks the active brush and nothing else
`#D9744A` SHALL indicate the active brush and the active tool state. It SHALL NOT be used for panel chrome, headings, borders, ordinary selection, hover states, or decoration.

#### Scenario: One accent on screen
- **WHEN** the application is displaying a document with a brush selected
- **THEN** the accent appears on the active brush and tool indication, and nowhere else

#### Scenario: Selection is not accented
- **WHEN** a layer or scene entry is selected
- **THEN** it is indicated by surface tone and weight, not by the accent color

### Requirement: The viewport stays neutral and desaturated
The viewport ground SHALL be neutral and desaturated so that the sculpted material reads truthfully. It SHALL NOT carry a colored tint, gradient, vignette or patterned backdrop that would alter the apparent shading of the surface.

#### Scenario: The material reads the same against the ground
- **WHEN** the same MatCap-shaded surface is displayed against the viewport ground and against a neutral reference
- **THEN** its apparent value is not shifted by the ground

### Requirement: Typography separates labels from values
Interface labels SHALL be set in a humanist sans face at a discreet weight and size. Numeric readouts — polygon, vertex and triangle counts, sizes, strengths, memory figures, coordinates — SHALL be set in a monospaced face so that digits align and changing values do not reflow.

#### Scenario: Numbers do not reflow as they change
- **WHEN** a numeric readout changes while the user drags a control
- **THEN** the digits occupy fixed positions and surrounding layout does not shift

#### Scenario: Section headings are discreet
- **WHEN** a panel section heading is displayed
- **THEN** it is set small, spaced and low-contrast against the panel surface rather than as a prominent title

### Requirement: The style budget is enforced
The interface SHALL hold to the stated style ratio: predominantly minimal flat surfaces, with skeuomorphic treatment confined to the brush swatches, material previews and the pressure control; sparing use of the space-UI register for the navigation gizmo and axis indicators; and HUD treatment confined to the viewport overlays. Skeuomorphic rendering SHALL NOT be applied to panels, buttons, sliders, menus or list rows.

#### Scenario: Panels stay flat
- **WHEN** any panel, button, slider or menu is rendered
- **THEN** it carries no bevel, embossing, gloss, drop shadow or simulated material

#### Scenario: Brush and material previews are physical
- **WHEN** brush swatches and material previews are rendered
- **THEN** they are shaded three-dimensional spheres that read as physical objects, which is the intended location of the skeuomorphic budget

### Requirement: Controls are quiet until addressed
Controls SHALL present at low contrast when inactive and gain contrast on hover, focus and while being adjusted. The interface SHALL NOT compete with the sculpt for attention while the user is not addressing it.

#### Scenario: Hover raises contrast
- **WHEN** the pointer enters a control
- **THEN** that control's contrast increases and no other control changes

#### Scenario: Sliders read as a track and a position
- **WHEN** a slider is displayed
- **THEN** it is drawn as a thin track with a position marker and its numeric value, without a raised handle or a filled decorative bar

### Requirement: Spacing and sizing come from a defined scale
Spacing, control heights, icon sizes and corner radii SHALL be drawn from a defined scale rather than chosen per component, so that panels align across the application.

#### Scenario: Panels align across regions
- **WHEN** the left and right panel regions are displayed together
- **THEN** their section headings, row heights and internal padding align on the same scale

### Requirement: Iconography is a single consistent set
Tool and panel icons SHALL come from one line-based set sharing stroke weight, corner treatment and optical size, drawn in the foreground token and accented only where a tool is active.

#### Scenario: Icons share weight
- **WHEN** icons from the tool rail, the toolbar and the brush shelf are displayed together
- **THEN** their stroke weight and optical size match

### Requirement: Every brush swatch carries a mark of its own
Each brush swatch on the shelf SHALL carry a drawn mark depicting the brush's
effect on a surface, distinct from every other brush's mark, drawn from the
same line-based set at one stroke weight and contained within the swatch.
Hovering a swatch SHALL show the brush's name and one sentence saying what it
does, in the interface's language. The mark SHALL NOT use the accent.

#### Scenario: Two brushes are told apart without their labels
- **WHEN** the shelf is displayed with the labels covered
- **THEN** no two swatches show the same mark

#### Scenario: A swatch explains itself on hover
- **WHEN** the pointer rests on a swatch
- **THEN** the brush's name and a one-sentence description appear, translated to the interface's locale

### Requirement: Interface text meets a stated contrast floor
Text and essential interface indicators SHALL meet at least a 4.5:1 contrast ratio against their background, and non-text indicators essential to understanding state SHALL meet at least 3:1. Where the quiet-until-addressed rule would fall below these floors, the floor SHALL win.

#### Scenario: A low-contrast label still passes
- **WHEN** an inactive secondary label is rendered at its resting contrast
- **THEN** its ratio against the panel surface is at least 4.5:1

#### Scenario: State is not conveyed by color alone
- **WHEN** the active brush is indicated
- **THEN** it is distinguished by more than the accent hue, so that the state survives a color-vision deficiency
