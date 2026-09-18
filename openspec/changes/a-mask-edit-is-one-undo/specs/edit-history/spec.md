## ADDED Requirements

### Requirement: An edit to the mask is one step of the history the interface reads
The history a user steps through is the one the application keeps, which counts
**actions** and remembers how many engine entries each action spent. Every edit
to a mask — inverting, clearing, expanding, contracting, smoothing, the bounded
complement, a region drawn round the form, and pulling the frozen patch off as
a wall — SHALL bank exactly one action on that history, whatever it cost
underneath.

What an edit cost SHALL be read from the history either side of it rather than
assumed to be one entry, and an edit that wrote nothing SHALL bank nothing.

Every path that writes to a mask SHALL bank through one place, so that an
operation added later cannot arrive without an entry.

#### Scenario: Each mask operation is one thing to take back
- **WHEN** the user applies any operation from the mask menu, draws a region or
  extrudes the frozen patch
- **THEN** the history offers exactly one more thing to undo than it did before

#### Scenario: An undo after a mask edit takes back the mask edit
- **WHEN** the user removes a subtool, paints a mask, strokes the clay, clears
  the mask and undoes once
- **THEN** the frozen region comes back, the clay the stroke moved is where the
  stroke left it, and the subtools are the ones the document had

#### Scenario: A redo puts the operation back
- **WHEN** the user takes a mask operation back and redoes
- **THEN** the mask is as the operation left it

#### Scenario: An operation that wrote nothing is not something to take back
- **WHEN** the user clears a mask that freezes nothing
- **THEN** the history offers exactly what it did before, and the user is told
  there was nothing to clear rather than being refused

### Requirement: A history step refreshes what is drawn of the mask
A mask is drawn but is not geometry, so nothing the surface reports tells the
viewport that a frozen region changed; every site that writes to a mask says so
beside the write. The engine's history writes to a mask through a snapshot of
its own, past those sites, so a step through the history SHALL itself say that
the frozen region may have moved.

#### Scenario: An undone mask operation is redrawn
- **WHEN** the user applies a mask operation and undoes it
- **THEN** the surface is drawn with the mask the undo restored rather than the
  one the operation left
