## MODIFIED Requirements

### Requirement: Every tool maps to a documented engine verb
Each sculpting tool the interface presents SHALL correspond to a documented
ClayCore verb reached through the C ABI. The application SHALL NOT present a
tool that has no engine counterpart, and SHALL NOT bind a label to a verb whose
behavior differs from what the label states.

Where a tool applies SHALL be declared once, per tool and per representation,
in the table the shelf, the availability check, the diagnostics report and the
tests all read. Nothing else may decide where a tool applies.

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

## ADDED Requirements

### Requirement: A brush colour is chosen and reaches the colour brushes
The application SHALL hold one current brush colour, shared across tools rather
than stored per tool, together with a short list of recently used colours. The
interface SHALL offer a swatch to choose it, shown when the active tool writes
colour and hidden when it does not.

The colour SHALL reach the engine as the palette entry a voxel paint brush
deposits and as the colour a mesh paint stamp blends toward. Painting SHALL
change colour and SHALL NOT move the surface.

Choosing a colour already in a grid's palette SHALL reuse that entry rather
than adding a duplicate.

#### Scenario: Painting a grid changes what is drawn
- **WHEN** the user picks a colour and paints across a voxel layer
- **THEN** the painted cells carry that colour, the rendered image changes, and
  no vertex position moves

#### Scenario: A masked region keeps its colour
- **WHEN** a region of a voxel layer is masked and a paint stroke crosses it
- **THEN** the frozen cells keep the colour they had

#### Scenario: One paint gesture is one undo
- **WHEN** a paint stroke is made and then undone
- **THEN** the previous colours come back in one step, and redo puts the new
  ones back

#### Scenario: A painted colour survives the document
- **WHEN** a voxel layer is painted, saved, closed and opened again
- **THEN** the colours are the ones that were painted

#### Scenario: The swatch is offered only where it is read
- **WHEN** a tool that does not write colour is active
- **THEN** the colour swatch is not shown

### Requirement: A voxel drag accumulates below the cell size
Where a drag verb resamples occupancy per cell, the application SHALL
accumulate the gesture's displacement from its anchor and SHALL issue only the
part that has grown past a whole cell, rather than passing raw pointer deltas
that would round to no movement.

The displacement issued SHALL be measured from the gesture's anchor, so that a
slow drag and a fast one over the same path move the material equally far. One
drag SHALL be one undo entry.

Under symmetry both the drag's centre and its direction SHALL be reflected.

#### Scenario: A slow drag still moves the material
- **WHEN** a voxel layer is dragged with Mover in steps smaller than one cell
- **THEN** the material moves once the accumulated displacement passes a cell,
  and ends up where a single drag of the same total would have put it

#### Scenario: A drag differs from a smudge
- **WHEN** the same gesture is made with Mover and with Nudge on a voxel layer
- **THEN** Mover carries the body of the form and Nudge drags only its skin

#### Scenario: A mirrored drag pulls both sides outward
- **WHEN** X symmetry is on and a voxel layer is dragged away from the mirror
  plane
- **THEN** both sides move away from the plane, rather than both moving the same
  way in world space
