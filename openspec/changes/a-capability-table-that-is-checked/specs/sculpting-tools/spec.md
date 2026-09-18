## ADDED Requirements

### Requirement: The capability table is checked against the engine that is linked
Every engine entry point the capability table names SHALL be a symbol the
build's own generated bindings declare, and every offered pair of tool and
representation SHALL reach an entry point its row names.

Both are properties of the table against the *engine*, and neither can be
asserted where the table lives: the domain links no engine and holds its verbs
as text. The list of declared entry points SHALL therefore be published by the
bridge, generated from the bindings rather than transcribed from the header, so
that what is checked is the ABI this build links.

What a stroke called SHALL be recorded where every fallible engine call already
passes, so the recorded name is the name that call would carry in its own error
and cannot fall out of step with it. The record SHALL NOT be compiled into a
build that does not ask for it.

A row MAY name more than one entry point, because more than one is reachable —
a field drag opens a transaction where it can and falls back where it cannot —
and the check is that the stroke reached one of them.

#### Scenario: An engine release renames a verb
- **WHEN** the engine pin moves and an entry point the table names is no longer
  declared
- **THEN** a test fails naming that entry point, rather than the row going on
  describing a call nobody makes

#### Scenario: A tool is rerouted to another entry point
- **WHEN** a tool's dispatch is changed to call an entry point its row does not
  name
- **THEN** a test fails naming the tool, the representation, what the row claims
  and what the stroke called

#### Scenario: A row that is right about the call
- **WHEN** every offered pair is stroked on a fixture of its representation
- **THEN** each one reaches an entry point its own row names

### Requirement: Each caveat about a representation is measured
Every caveat the application attaches to a tool on one representation SHALL
have a test measuring the difference it describes, and adding a caveat without
one SHALL NOT compile.

A caveat is shown to an artist as a fact about the engine's vocabulary. One
that is not measured is a sentence the interface tells because it reads well,
and the smoothing caveat on a hierarchy is exactly that today: it describes a
call no stroke opens.

A caveat that is not yet true SHALL be pinned by a test that fails when it
becomes true, so that closing the gap is announced rather than silent.

#### Scenario: A caveat is added without a measurement
- **WHEN** a caveat is added to the set the application can show
- **THEN** the test that names a measurement for each one stops compiling until
  it is given one

#### Scenario: A grid's flatten is two-sided
- **WHEN** the flatten is stroked across material a plane passes through on a
  grid
- **THEN** cells that were empty below the plane are filled as well as cells
  above it removed, where the scrape given the same plane fills none

#### Scenario: A hierarchy has no colour to write
- **WHEN** a colour brush is applied to a hierarchy layer
- **THEN** it is refused, where the same brush on the mesh the note sends an
  artist to is applied

## MODIFIED Requirements

### Requirement: Every tool maps to a documented engine verb
Each sculpting tool the interface presents SHALL correspond to a documented
ClayCore verb reached through the C ABI. The application SHALL NOT present a
tool that has no engine counterpart, and SHALL NOT bind a label to a verb whose
behavior differs from what the label states.

Where a tool applies SHALL be declared once, per tool and per representation,
in the table the shelf, the availability check, the diagnostics report and the
tests all read. Nothing else may decide where a tool applies.

A row SHALL name the entry point that executes for that pair, spelled in full.
A family abbreviated to one name plus suffixes — `begin/update/commit` — is a
name that cannot be looked up, and a row that names the kind of call rather than
the call is a row nothing can check. Where the same verb reaches the engine
through a resolved stroke for one tool and a single stamp for another, the rows
SHALL differ accordingly.

Beyond the vocabulary already bound, the declared table SHALL include:

- **Mover** on voxel layers, through the grid's grab verb.
- **Planar** on voxel layers, through the grid's flatten verb, which is
  two-sided where the SDF and mesh sides are cut-only.
- **Vinco** on SDF layers, through the incise operation.
- **Argila** on SDF layers, through the relief operation with buildup
  accumulation.
- **Mover Topológico**, on SDF layers only, through the engine's topological
  move — a drag whose falloff is measured along the material rather than
  through space.

A tool SHALL NOT be offered on a representation whose engine verb this
application does not reach, and a declared pair SHALL reach a distinct engine
call rather than falling through to a neighbouring one.

#### Scenario: A tool's label matches its verb
- **WHEN** the user selects Planar and applies it to a surface
- **THEN** the engine's flatten operation runs — cut-only on a field or a mesh,
  and two-sided on a grid, which is the verb the grid has

#### Scenario: Padrão and Inflar leave different marks on a field
- **WHEN** the same stroke is made on an SDF layer with Padrão and with Inflar
- **THEN** the two surfaces differ: Padrão's mark is a ridge following the
  falloff, Inflar's a broader swelling of the footprint

#### Scenario: No orphan tools
- **WHEN** the tool registry is enumerated
- **THEN** every entry names the engine entry point it invokes, and none is unbound

#### Scenario: Every declared pair reaches its own verb
- **WHEN** each tool is applied on each representation its row declares
- **THEN** the edit lands, and no two tools on one representation resolve to the
  same engine call unless the table says they do

#### Scenario: A row names the call that runs
- **WHEN** a row names an entry point and the dispatch for that pair calls
  another
- **THEN** the row is wrong, whether or not the name it carries is a symbol the
  engine has

#### Scenario: Crease cuts a trough on a field
- **WHEN** Vinco is stroked across an SDF surface
- **THEN** a narrow trough is displaced into the accumulated surface through the
  incise operation, and no new primitive is added to the layer

#### Scenario: Crease inverted raises the ridge it would have cut
- **WHEN** Vinco is stroked with the invert modifier held on an SDF layer
- **THEN** the surface rises where the upright stroke would have cut, which is
  the operation the engine names as incise's inverse

#### Scenario: Clay builds up where a stroke crosses itself
- **WHEN** Argila is stroked twice over the same place on an SDF layer
- **THEN** the second pass adds to the first, and the same two passes with
  Camada do not, because Camada is the clamped-accumulation tool

#### Scenario: A topological drag does not reach across a gap
- **WHEN** Mover Topológico is dragged on a form whose two parts are close in
  space and far along the surface
- **THEN** only the part under the brush moves, where the Euclidean Mover at the
  same radius moves both
