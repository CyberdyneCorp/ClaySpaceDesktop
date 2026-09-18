## ADDED Requirements

### Requirement: Inflate and Pinch on a field are one signed radial scale
`Inflar` and `Pinçar` on an SDF layer SHALL both reach the engine's magnify of
the assembled surface, `Inflar` at a positive strength and `Pinçar` at a
negative one. Neither SHALL be bound to the relief operation, which is the verb
`Padrão` is: relief moves the accumulated surface along its own normal, so two
brushes bound to it are one verb wearing two footprints however the footprints
are shaped.

`Pinçar` SHALL be offered on a field. It was absent because the per-item
magnify gathers one contributor of a smooth union and leaves the rest; the
assembled-surface entry point resolves the region against every item it
reaches, which is what makes the tool possible there at all.

The region's radius SHALL come from the brush size and its easing from the drag
falloff, a magnify being a region deformation rather than a stamp. The
magnitude SHALL come from Intensidade, and the radius and magnitude together
SHALL be chosen so that the swell is **lower at its peak and wider in its
footprint** than the ridge the same brush draws with `Padrão`. A magnitude that
makes the swell taller as well as wider is a bigger brush rather than a
different one, and a radius no larger than the brush's is not wider at all.

A radial scale fixes its own centre: the point the region is centred on does
not move and the points nearest it barely do. A gesture's samples are raycast
hits, so left alone every dab is centred exactly where the verb has least to
say. `Inflar`'s dabs SHALL therefore be sunk into the material, along the
field's own gradient, where a scale has clay all round it to push outward.
`Pinçar`'s SHALL be left on the surface, because a gather about a point on the
surface draws the material toward the stroke — which is what pinching is —
where a sunk gather deflates uniformly instead.

The depth is a property of the tool and not of the sign. The invert key SHALL
therefore give each tool its own opposite rather than the other tool: an
inverted `Inflar` deflates and an inverted `Pinçar` spreads, which is the pair
the grid's column already names for these two.

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

#### Scenario: Inflate is broader and lower than Standard
- **WHEN** the same stroke is made on an SDF layer with `Padrão` and with
  `Inflar`, at the same size and the same intensity
- **THEN** the swell peaks lower than the ridge and reaches further to the side
  of the stroke

#### Scenario: Pinch gathers the surface toward the stroke
- **WHEN** `Pinçar` is stroked across an SDF surface
- **THEN** the surface under the stroke stands proud and the rim of the
  region falls away, the material having moved toward the stroke

#### Scenario: Inverting a magnify brush gives its own opposite
- **WHEN** `Inflar` and `Pinçar` are each stroked with the invert modifier held
- **THEN** the inverted swell hollows the surface under the stroke, and the
  inverted gather raises it across the whole footprint and nowhere lowers it

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
- **Inflar** and **Pinçar** on SDF layers, through the magnify of the
  assembled surface: one entry point at a positive strength and at a negative
  one.

A tool SHALL NOT be offered on a representation whose engine verb this
application does not reach, and a declared pair SHALL reach a distinct engine
call rather than falling through to a neighbouring one.

#### Scenario: A tool's label matches its verb
- **WHEN** the user selects Planar and applies it to a surface
- **THEN** the engine's flatten operation runs — cut-only on a field or a mesh,
  and two-sided on a grid, which is the verb the grid has

#### Scenario: Padrão and Inflar leave different marks on a field
- **WHEN** the same stroke is made on an SDF layer with Padrão and with Inflar
- **THEN** the two surfaces differ, and they differ by verb rather than by
  footprint: Padrão's mark is a ridge the relief operation displaces along the
  surface's own normal, Inflar's a broader and lower swell a radial scale
  spreads

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
