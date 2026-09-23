## MODIFIED Requirements

### Requirement: Adding a level is priced and refused rather than attempted
The application SHALL state what adding a level would cost before it is added,
and SHALL refuse a level that does not fit. The figure stated SHALL be the
**peak** during the build rather than what remains after it, because on a
constrained machine it is the high-water mark that ends the session.

Whether a level fits SHALL be decided on top of what the document already
holds, and a refusal SHALL name what is held, the peak and the budget.

A refused level SHALL leave the hierarchy exactly as deep as it was.

#### Scenario: The cost is beside the offer
- **WHEN** a hierarchy is the active layer
- **THEN** what one more level would occupy is shown beside the control that
  would add it, without adding one

#### Scenario: A level that does not fit is refused
- **WHEN** the user asks for a level whose peak, added to what the document
  already holds, exceeds the budget
- **THEN** the request is refused with what is held, the peak and the budget
  stated, and the hierarchy holds the levels it held before

#### Scenario: The refusal reaches the screen
- **WHEN** an operation on the active layer is refused
- **THEN** the reason is shown beside the viewport rather than only recorded

### Requirement: A hierarchy carries passes that stay adjustable
The application SHALL let the sculptor make named passes on a subdivision
hierarchy, send a stroke into one, and afterwards dial its strength, hide it,
lock it, reorder it, fold it into the pass below, bake it into the form, or
remove it.

A pass's strength SHALL remain adjustable for as long as the pass exists,
independently of the gesture that filled it. Dialling a pass SHALL replay no
stroke: a pass at zero strength SHALL contribute exactly nothing, and returning
it to full SHALL restore exactly what was there.

Acting on a pass SHALL NOT enter the edit history. A pass is a property of the
stack rather than a step in the work.

The bottom pass SHALL NOT be folded into the pass below, because there is none:
the form under the passes is not a pass. Asking for it SHALL be refused with a
sentence that names baking into the form as what the pass can do instead, and
SHALL NOT surface the engine's result code.

#### Scenario: A pass is dialled long after the stroke that filled it
- **WHEN** the user strokes into a pass, releases the pointer, and later moves
  that pass's strength to zero
- **THEN** the surface returns to what it was before the stroke, and moving the
  strength back restores the stroke exactly

#### Scenario: Hiding a pass removes its contribution
- **WHEN** the user hides a pass
- **THEN** the surface is exactly what it would be with that pass at zero
  strength

#### Scenario: Undo does not take back a slider
- **WHEN** the user dials a pass and then undoes
- **THEN** the last edit is undone, and the pass keeps the strength it was
  given

#### Scenario: The bottom pass has nothing to merge into
- **WHEN** the user asks for the bottom pass to be merged down
- **THEN** the request is refused with a sentence that names baking into the
  form, every pass is still on the stack, and a pass above it still merges down
