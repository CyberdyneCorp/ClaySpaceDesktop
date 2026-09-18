## ADDED Requirements

### Requirement: The inspector answers what is being sculpted
The right region SHALL carry one section describing the active layer's
representation, in a fixed position. Its contents SHALL change with the
representation; its position SHALL NOT, so that the sections around it stay
where a sculptor left them.

The section SHALL be headed by the representation it describes, and that
heading SHALL differ from every other heading the region draws. Section folds
are keyed by the heading's word, so two sections sharing one would share a
fold.

The section SHALL be drawn only where it has something to say. A heading with
no body SHALL NOT be drawn.

#### Scenario: The section names the representation
- **WHEN** a layer of any representation is active
- **THEN** the right region carries a section headed with that representation's name

#### Scenario: Folding one section does not fold another
- **WHEN** the user folds the geometry section on a grid layer
- **THEN** the grid's own section stays open

#### Scenario: The panel does not rearrange
- **WHEN** the user makes a layer of a different representation active
- **THEN** only the contextual section's contents change, and the sections above and below it keep their order

#### Scenario: Nothing to say draws nothing
- **WHEN** the engine has reported nothing about the active field layer
- **THEN** no field section is drawn, rather than a heading over an empty body

### Requirement: The inspector exposes only what the domain holds
The contextual section SHALL offer controls and readouts only for values this
application's domain or the engine can actually express for that layer. It
SHALL NOT present a control for a setting nothing reads, whatever a design
reference depicts.

Where a representation-specific control already exists elsewhere for a stated
reason — belonging to the stroke rather than the layer, or standing beside the
thing it acts on — it SHALL NOT be duplicated here.

#### Scenario: A depicted control with no domain behind it is absent
- **WHEN** a design reference shows a per-layer setting the domain cannot express
- **THEN** no control for it is drawn

#### Scenario: A field states its edit list
- **WHEN** a field layer is active and the engine has reported on it
- **THEN** the section states how many items the list holds and whether it has been collapsed

#### Scenario: A mesh states its topology contract
- **WHEN** a mesh layer is active
- **THEN** the section states that its brushes move existing vertices and neither add nor remove any

#### Scenario: A control that lives elsewhere is not repeated
- **WHEN** a representation's control already stands in the options bar or beside the layer stack
- **THEN** the contextual section does not draw a second copy of it
