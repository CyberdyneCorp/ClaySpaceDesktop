## ADDED Requirements

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

### Requirement: Where a stroke lands is a row the sculptor selects
The interface SHALL show the stack of passes under the layer they stand on, and
SHALL show the form beneath them as a row of its own. Exactly one of those rows
SHALL be selected at a time, and the next stroke SHALL enter whichever it is.

Selecting the form SHALL leave every pass untouched, so that a sculptor can
correct the surface under a set of passes without disturbing them.

A hierarchy with no passes SHALL still show the form's row, so that the
sculptor can see where a stroke is going before there is anywhere else for it
to go.

#### Scenario: A new pass takes the next stroke
- **WHEN** the user adds a pass
- **THEN** it is selected, and the next stroke goes into it

#### Scenario: The form is selected and the passes are left alone
- **WHEN** the user selects the form's row and strokes
- **THEN** the surface under the passes changes and every pass keeps exactly
  what it held

### Requirement: Reordering a pass is organisation and never geometry
The interface SHALL let the sculptor reorder passes by dragging a row, and that
reorder SHALL move no vertex: the passes compose as a sum, so their order
decides where a row is drawn and never what the surface is.

#### Scenario: A drag reorders the list and nothing else
- **WHEN** the user drags one pass onto another
- **THEN** the two swap places in the list and the surface is unchanged

### Requirement: The composition is held while a stroke is open
While a gesture is in progress the application SHALL refuse the changes that
would recompose the surface — a strength, a visibility, a reorder, an addition,
a removal — and SHALL say that it is waiting for the pointer rather than
failing silently. A rename, a lock and a change of which row is selected move
no vertex and SHALL be accepted.

#### Scenario: A slider moved mid-gesture is refused with a reason
- **WHEN** the user moves a pass's strength while a stroke is still open
- **THEN** the change is refused, the interface says the brush has to be
  released, and the change works as soon as it is

### Requirement: What the stack costs is shown, and nothing is enforced against it
The interface SHALL show what the stack of passes occupies and how much surface
it covers, and SHALL offer to release the storage a stroke that undid itself
left behind. No limit SHALL be enforced against that figure: a cap that
silently stopped recording would leave a pass on the surface and un-dialable.

#### Scenario: A large stack is called out rather than capped
- **WHEN** the stack passes the size at which it is worth releasing
- **THEN** the figure is drawn in the interface's warning colour and the offer
  to release stands, and every pass goes on taking strokes
