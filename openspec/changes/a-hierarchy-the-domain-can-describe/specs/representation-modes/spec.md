## MODIFIED Requirements

### Requirement: The active representation is visible without inspection
The application SHALL show the active layer's representation in the viewport
chrome and in the layer stack, at all times and without the user opening a
panel. The representations SHALL be distinguishable from each other by more than
colour alone.

#### Scenario: The representation is on screen
- **WHEN** a layer is active
- **THEN** its representation is named in the viewport chrome and beside the
  layer in the stack

#### Scenario: Switching layers changes what is shown
- **WHEN** the user makes a layer of a different representation active
- **THEN** the displayed representation changes to match, in both places

#### Scenario: A subdivision hierarchy is one of them
- **WHEN** the set of representations the application knows about is listed
- **THEN** a subdivision hierarchy is among them, with a name, a phrase saying
  what it is, an icon of its own shape and a short tag, in every language the
  interface offers

## ADDED Requirements

### Requirement: A hierarchy's sculpt level and display level are independent
The application SHALL model, for a layer holding a subdivision hierarchy, the
level a stroke writes on and the level the viewport draws as two separate
quantities. Moving one SHALL NOT move the other. Adding a level SHALL move both
to the level it added, which is what an artist means by subdividing.

#### Scenario: Moving the brush leaves the viewport where it was
- **WHEN** the sculpt level is changed on a hierarchy whose display level is
  finer
- **THEN** the display level is unchanged, and nothing is redrawn

#### Scenario: Moving the viewport leaves the brush where it was
- **WHEN** the display level is changed
- **THEN** the sculpt level is unchanged

#### Scenario: Subdividing moves both
- **WHEN** a level is added
- **THEN** both the sculpt level and the display level are the new level

#### Scenario: The interface can say the two disagree
- **WHEN** the sculpt level and the display level are different
- **THEN** the application can report that what is drawn is not what is being
  written

### Requirement: A hierarchy carries a stack of passes addressed by identity
The application SHALL model a hierarchy's sculpt layers as named, reorderable
passes carrying a strength, a visibility, a lock and whether they hold a stored
mask. Each pass SHALL be addressed by an identity that survives a reorder, and
SHALL NOT be addressed by its position in the stack.

#### Scenario: A reorder moves no vertex
- **WHEN** a pass is slid to another position in the stack
- **THEN** what the passes contribute to the surface is unchanged, and the
  application does not treat the reorder as an edit to the surface

#### Scenario: An identity outlives a reorder
- **WHEN** a pass is slid to another position
- **THEN** the same identity still names the same pass, with the same strength,
  visibility, lock and coverage

#### Scenario: A hidden pass contributes exactly nothing
- **WHEN** a pass is hidden
- **THEN** its contribution is zero rather than nearly zero

#### Scenario: A lock refuses a write and permits every property change
- **WHEN** a pass is locked
- **THEN** a stroke, a merge and a bake aimed at it are refused, and its name,
  strength, visibility and lock can still be changed

#### Scenario: A pass that is gone does not keep a stroke aimed at it
- **WHEN** the stack is read back without the pass that was active
- **THEN** the next stroke is routed into the form under the passes

### Requirement: Where a stroke on a hierarchy would land is chosen rather than inferred
The application SHALL let the destination of a stroke on a hierarchy be chosen
between the active pass, the form under the passes, and whichever of those two
applies, and SHALL be able to answer which of them a stroke would enter before
one is made. A destination naming the active pass where there is none SHALL be
answered as a refusal rather than as the form.

#### Scenario: With no destination chosen and no passes, a stroke enters the form
- **WHEN** a hierarchy carries no passes and no destination has been chosen
- **THEN** the application answers that a stroke would enter the form under the
  passes

#### Scenario: Choosing the form answers the form, whatever is active
- **WHEN** the destination is the form and a pass is active
- **THEN** the application answers that a stroke would enter the form

#### Scenario: Choosing a pass that is not there is a refusal, not a fallback
- **WHEN** the destination is the active pass and no pass is active
- **THEN** the application answers that the gesture would be refused, rather
  than answering with the form
