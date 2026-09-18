## ADDED Requirements

### Requirement: Undo costs what the edit cost
Taking a step through the history SHALL re-mesh the region that step actually
changed, and not the whole of any layer. The application SHALL take the
region from the engine, which reports it, rather than deriving one.

The region is a world region and not a layer's, so a step SHALL be re-meshed
wherever it landed, including on a subtool that is not the active one.

Where the engine reports no finite region — a non-local operation, an
unbounded primitive — the application SHALL fall back to dirtying everything
rather than guessing a box. Where it reports that nothing changed, the
application SHALL re-mesh nothing.

#### Scenario: An undo is as cheap as the dab it reverses
- **WHEN** the user undoes a brush dab
- **THEN** the surface re-meshed is the dab's own neighbourhood, not the
  layer's, and the undo costs about what the dab cost

#### Scenario: An undo on another subtool is drawn
- **WHEN** the user makes an edit on one subtool, activates another, and undoes
- **THEN** the edit is taken back *and* the subtool it was made on is
  re-meshed, rather than the active one

#### Scenario: An unbounded step still dirties everything
- **WHEN** the step undone cannot be bounded by a finite region
- **THEN** the whole layer is re-meshed rather than a guessed region
