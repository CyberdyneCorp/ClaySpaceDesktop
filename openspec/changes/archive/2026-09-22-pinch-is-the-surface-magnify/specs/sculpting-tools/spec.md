## ADDED Requirements

### Requirement: Pinch on a field is a radial scale of the assembled surface
`Pinçar` on an SDF layer SHALL reach the engine's magnify of the assembled
surface at a **negative** strength, which gathers the region toward the dab's
centre. It SHALL NOT be bound to a stroke operation: relief and incise move the
surface along its own normal, and no shaping of that profile is a gather.

`Pinçar` SHALL be offered on a field. It was absent because the per-item
magnify gathers one contributor of a smooth union and leaves the rest; the
assembled-surface entry point resolves the region against every item it
reaches, which is what makes the tool possible there at all.

The region's radius SHALL come from the brush size and its easing from the drag
falloff, a magnify being a region deformation rather than a stamp. The
magnitude SHALL come from Intensidade, and the region SHALL be wider than the
brush, a gather having nothing to gather from within it otherwise.

A radial scale fixes its own centre: the point the region is centred on does
not move and the points nearest it barely do. The dab SHALL be left standing on
the surface, where the gesture's raycast put it, because a gather about a point
on the surface draws the material toward the stroke — which is what pinching is
— where a gather about a point sunk into the material deflates uniformly
instead.

The invert key SHALL spread: the material leaves the stroke instead of arriving
at it, which is the pair the grid's column already names for this tool.

A stroke SHALL lay one dab per step of the brush's spacing along the path,
rather than one per sample: the engine folds frames that share a centre, so a
pointer resting still would otherwise pile a gesture's worth of frames at one
place and keep only the last.

Under symmetry the gesture SHALL be applied once with the layer's mirror
pointed, rather than reflected and applied again. The engine reflects the
region into every image the layer emits and carries the strength across each
one untouched — a reflection of a radial scale is a radial scale of the same
strength — where a drag's displacement has to be mapped per image.

The frozen region SHALL be honoured by the stroke. The engine's descriptor
carries no gate, so samples the mask protects SHALL be dropped from the path
before any dab is placed.

The whole gesture SHALL be one step of the history the sculptor presses,
however many dabs it laid down.

#### Scenario: Pinch gathers the surface toward the stroke
- **WHEN** `Pinçar` is stroked across an SDF surface
- **THEN** the surface under the stroke stands proud and the rim of the
  region falls away, the material having moved toward the stroke

#### Scenario: Inverting the gather spreads
- **WHEN** `Pinçar` is stroked with the invert modifier held
- **THEN** the surface rises across the whole footprint and nowhere falls

#### Scenario: A magnify across a blend moves both contributors
- **WHEN** a magnify is stroked over the join of a form made of two
  smooth-unioned items
- **THEN** the surface moves on both sides of the blend rather than on one

#### Scenario: A gesture and its mirror are the same field
- **WHEN** a magnify is made with a symmetry axis on
- **THEN** the surface on the reflected side is left where the stroke's own
  side is

#### Scenario: One stroke is one undo
- **WHEN** a stroke long enough to lay down several dabs is made and then
  undone once
- **THEN** the whole stroke is taken back

### Requirement: A field's Standard says what its operation is faithful to
`Padrão` and `Inflar` on an SDF layer SHALL both remain bound to the relief
operation, differing in footprint and lift alone. Relief offsets the
accumulated field, so every point of the isosurface moves along the field's own
gradient; that is the **Inflate** frame, and the engine's own measurement of it
against frame-isolated references leaves nothing for a different operation to
improve on.

Because the same operation is therefore an approximation of Standard, `Padrão`
on a field SHALL carry a tool note stating the approximation and what decides
how far off it is: the spread of the normals under the stamp, which is a few
percent of the amplitude on a form smooth at the brush's scale and the whole
amplitude on a feature narrower than the stamp. The note SHALL offer the
remedy, which is a brush smaller than the feature.

The claim the note makes SHALL be held by a measurement rather than by the
prose, as every tool note's is.

#### Scenario: A thin feature takes the mark on its flanks
- **WHEN** one `Padrão` stamp is made on the top of a fin thinner than the
  brush, and on a sphere several times the brush
- **THEN** the fin grows sideways by nearly as much as its top rose, and the
  sphere does not, which is the divergence from a displacement along one
  averaged normal

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
- **Pinçar** on SDF layers, through the magnify of the assembled surface at a
  negative strength.

A tool SHALL NOT be offered on a representation whose engine verb this
application does not reach, and a declared pair SHALL reach a distinct engine
call rather than falling through to a neighbouring one.

#### Scenario: A tool's label matches its verb
- **WHEN** the user selects Planar and applies it to a surface
- **THEN** the engine's flatten operation runs — cut-only on a field or a mesh,
  and two-sided on a grid, which is the verb the grid has

#### Scenario: Padrão and Inflar leave different marks on a field
- **WHEN** the same stroke is made on an SDF layer with Padrão and with Inflar
- **THEN** the two surfaces differ: one operation, two profiles, Padrão's a
  ridge that follows the falloff and Inflar's a broader and lower swell

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
