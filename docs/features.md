# Features

What works today. Anything not listed here is not built yet — see
[roadmap.md](roadmap.md).

Four tools currently damage what they touch, and two controls do nothing.
Neither is a gap in this application: both are engine defects, filed, and
listed under *Known-degraded* at the end of this page. They are called out
there rather than quietly omitted, because a tool that is offered and does the
wrong thing is worse than one that is missing.

Every tool names the ClayCore entry point it invokes, so a binding can be
checked against the engine's own documentation without reading the
implementation. A tool with no engine counterpart is not offered.

## Sculpting tools

All fifteen are bound and each is covered by a before-and-after capture in
`target/visual/`.

| Tool | Engine verb | Layers | What it does |
|---|---|---|---|
| Padrão | `clay_layer_apply_stroke` with relief | both | Displaces the surface along its normal |
| Inflar | `clay_voxel_sculpt_inflate` / relief | both | Dilates; a negative amount erodes |
| Suavizar | `clay_item_volume_relax` / `clay_voxel_sculpt_smooth` | both | Relaxes the surface. Bakes on the field side |
| Mover | `clay_layer_move_surface` | SDF | Drags the assembled surface. Buds rather than stretches |
| Pinçar | `clay_voxel_sculpt_pinch` | voxel | Moves surface cells toward the brush centre |
| Raspar | `clay_voxel_sculpt_scrape` | voxel | Flattens and smooths from one snapshot |
| Planar | `clay_item_volume_flatten_from`, cut-only | SDF | Planes without filling, which keeps a facet crisp |
| Preencher | `clay_voxel_sculpt_fill_cavities` | voxel | Fills narrow pockets |
| Camada | `clay_layer_apply_stroke`, clamped | both | A stroke that does not build up on itself |
| Máscara | `clay_mask_apply_stroke` | both | Freezes a region against every verb. Invert, clear, expand, contract, smooth, bounded complement and extrude are in the Máscaras menu |
| Puxar | swept-sphere chain | SDF | Pulls a tendril out, tapering to its tip |
| Polir | `clay_item_volume_flatten_from`, cut-only | SDF | hPolish |
| Relaxar | `clay_item_volume_relax` | SDF | Relax as a brush |
| Nudge | `clay_voxel_sculpt_smudge` | voxel | Drags the surface skin, leaving the interior |
| Trim | `clay_cut_create` | SDF | A shape drawn on the frame, cutting through |

**Trim is not a stroke tool.** Its gesture is a shape drawn on the view frame,
not a drag across the surface, and the interface refuses a stroke for it rather
than doing something adjacent to what the label says.

**The shelf holds what the active layer's representation has.** Which tool
reaches which representation is a declared table rather than a rule written per
tool, and the shelf, the availability check and the tests all read it — so the
list you see and the list that works cannot drift apart. Eleven of the fifteen
have an SDF verb and nine a voxel one; a tool with no verb on the active
representation is *absent* rather than shown and greyed, because with three
vocabularies a single list would be mostly disabled rows all saying the same
sentence.

A tool that *does* have a verb here and still cannot be used — the layer is
locked, hidden, or missing an attribute the tool needs — is shown disabled and
says which of those it is. That is a different sentence and worth the space.

Changing the active layer keeps the active tool where the new representation
has it and substitutes one where it does not, saying so in the status line
rather than resetting silently. Brush settings are held per tool *and* per
representation: a size that suits a grid's cells is not the size that suits a
field, so returning to a tool on a layer returns the settings it had there.

Mesh layers currently offer no tools. That is a statement about this
application and not about the engine — see *Deliberately absent* — and the
table's own test fails the day it stops being true.

## Brush controls

| Control | Maps to | Range |
|---|---|---|
| Intensidade | stroke strength | 0–1 |
| Tamanho | stroke radius, in document units | 0.005–1 |
| Fluxo | stamp spacing | 0.01–1 |
| Ruído | positional jitter | 0–1 |
| Borda | falloff: Dura, Linear, Suave, Gaussiana | — |
| Acumular | buildup against clamped accumulation | on/off |
| Suavização | lazy-mouse lag | 0–0.95 |

Settings are held **per tool**: switching away and back returns what you left,
not a default. Values are clamped to what the engine accepts rather than
producing an error you cannot act on.

Symmetry about X, Y and Z is applied through the layer's mirror, so both halves
belong to one operation and undo together.

## Sculpting a mesh layer

A mesh layer comes from an import or from a **crossing** — see *Crossing
between representations*, where SDF and voxel layers both reach one. Either way
it is the same kind of thing from here on: the verbs reach both, the quality
readout measures both, and a save writes both.

**A drag pulls; it does not slide.** The interface picks the surface under the
pointer for a stamping verb, because that is where the stamp belongs. A
*dragging* verb takes hold once and then follows the pointer, carrying what it
took hold of along the plane it was picked on — which is what lets a drag leave
the form and pull a lobe out of it.

Picking every sample instead put every position *on* the surface, so the motion
between two of them was a walk along it: the skin stretched and folded and
nothing was carried anywhere, and a drag that crossed the silhouette stopped
sending samples at all. Against Blender's Grab, matched sphere and the same
1.737 drag at strength 0.65:

| delivery | furthest vertex moved | reached out to |
|---|---|---|
| picked every sample | 0.649 | 1.000 |
| carried | 1.128 | 1.617 |
| Blender | 1.129 | 1.508 |

Picked, the surface never leaves the unit sphere it started as.

Mover is applied as **one stamp at the anchor** rather than as a resolved
stroke, for the same reason: a stroke walks the brush centre along the path, so
a drag that leaves the surface takes the centre with it and the later stamps
reach no material. A single stamp reads the descriptor's own radius, strength
and direction — which a stroke ignores — so the region is the one under the
anchor and the displacement is the gesture's. Puxar and Nudge stay on the
stroke path deliberately: one re-anchors on every stamp so its region walks
with the pull, the other pushes along the surface.

**Every segment of a drag replays it from the anchor.** Mover and Puxar anchor
on the first stamp and carry that region by the motion that follows, so a
segment holding only the newest samples is a *second* grab anchoring where the
first stopped — on screen, a crumpled crease along the path where the form
should have been pulled. Measured against Blender's Grab over MCP, matched
sphere and the same drag:

| delivery | reached | moved the surface by |
|---|---|---|
| the gesture from its anchor | 9.8% | 0.707 |
| Blender | 11.4% | 0.779 |
| two independent segments | 19.0% | 0.569 |

Two anchors sharing one drag reach nearly twice as far and move less.

So the segments stay — they are what makes the drag visible while it happens —
and each replays the whole gesture instead. A replayed segment fires on **every
pointer move**, and a stamping one on a mesh every **one** stamp, where on a
field it waits for three: a
field segment costs a re-mesh of every brick it touched; a mesh one re-meshes
nothing, because the layer's own triangles are what the viewport reads. At the
default flow and a brush of 0.858 the field threshold is 1.03 world units —
most of the way across a unit sphere.

The bake-and-replace verbs are held whole on a field and **not** on a mesh:
Suavizar, Relaxar, Planar and Polir sample a region into a volume there and
segmenting that stacks a replacement per segment until the result crumbles,
while on a mesh they are ordinary stamps over the vertices in reach. Held whole
on a mesh, Suavizar arrived only when the pointer came up — which was half of
why it read as doing nothing.

It costs about 17 ms a move on a 140,774-vertex mesh, nearly all of it the
stamp itself rather than the buffer it fills (1.2 ms). Dropping the surface
walk would take it to 12.8, and is not taken: with the single-stamp path the
walk is what makes Move topological, and `mesh_move.rs` fails without it. The model takes back what the last
segment did before laying the gesture down again, which is what keeps one drag
to one undo: only the release banks anything, and a cancelled gesture takes its
preview with it.

**The pointer finds it from the moment it becomes active.** A pick against a
mesh layer is answered by the mesh sculptor's own raycast, and the sculptor was
built by the first stroke — but the interface places a stroke where the pick
reported and sends nothing where it reported nothing, so the first stroke could
never arrive. A mesh layer was unsculptable through the pointer, imported or
converted, and a press orbited the camera instead. The sculptor is built when
the layer is selected: a discrete action, where the adjacency pass it costs is
worth paying, rather than something a moving pointer repeats.

Sixteen fixed-topology verbs reach a mesh layer's own vertices, and
all sixteen hold one line above everything else: **topology never changes.** No
polygon is created, split or deleted, so `indices` and quads come out byte for
byte and a model that has just been retopologized can be refined without
spending the retopology.

**The falloff is measured along the surface, not through the air** — ZBrush's
*Move Topological*, and on a mesh it is the only kind there is. The engine's
brush descriptor carries a `geodesic` flag ("a brush on the upper lip must not
drag the chin through a closed mouth") and defaults it on; this application
sets it for every verb except Planar and Raspar, which mean "everything under
this disc" and whose surface walk would refuse to flatten across a groove.
`mesh_move.rs` measures it on a horseshoe whose tips are 0.71 apart through the
air and 2.36 apart around the arc: a brush reaching 1.0 drags one tip and
leaves the other where it was.

(`clay_item_volume_move_topological` is a different call and is not this one —
it takes an item carrying a volume and is refused on anything else, so it
belongs to the SDF side.)

**A mesh stroke never builds on itself — unless it is *converging*.** The field
and the grid are unaffected and Acumular means what it means there. Not a
preference: the verbs that displace along a *per-vertex* normal read the
normals the previous stamp just moved, so building up feeds a stamp's output
back into its own next input. A smoothing verb has the opposite character — it
averages toward the neighbourhood, so running it again moves less each time and
converges — and clamping one means a sculptor can never smooth more than a
single stamp's worth however long they rub. Suavizar, Relaxar and Polir are
exempt for that reason.

**Smoothing runs sixty-four Laplacian passes a stamp.** The engine's SMOOTH
averages a vertex with its *one-ring*, a high-frequency filter that takes out
tessellation noise and barely touches a bump spanning many edges. Measured on a
ridge standing 0.0676 proud of a unit sphere, four passes over it: at the
engine's own default the ridge came down by under one percent of its height; at
sixty-four it comes down by nearly three quarters. The passes are cheap next to
finding the region — 5.4 ms against 4.0 for a 0.18 brush on 140,774 vertices. Measured against Blender's brushes over MCP — matched sphere,
same brush radius in world units, same strength, same stroke — as the mean
angle between adjacent vertex normals before and after:

| verb | accumulating | clamped | Blender |
|---|---|---|---|
| Inflar | 5.04x | 1.18x | 1.00x |
| Pinçar | 9.41x | 1.83x | 1.00x |
| Vinco | 3.71x | 1.34x | 1.00x |
| Padrão | 1.11x | 1.08x | 1.00x |

Padrão is the control and barely moves either way: it uses the *region's*
averaged normal, so there is nothing to feed back.

Twelve of them are tools that already existed — a smooth is a smooth whichever
representation it lands on — and four are new: Argila, Vinco, Pintar and
Borrar. The two colour verbs refuse a mesh carrying no colour attribute rather
than creating one, because twelve bytes a vertex is a real cost to hide behind
a stroke.

**Malha aparente** (Visualizar, or Shift+F) draws the mesh's own edges over
it — ZBrush's polyframe. It answers the one question a shaded surface hides:
how much geometry is actually there. That is the question a crossing into a
mesh hands you, since what comes out is the sampling lattice's topology and
the density is the whole of what decides whether it wants retopology. The edges
are deduplicated before they are drawn: they are translucent, and an edge
emitted once per triangle would be blended twice, making the interior read
heavier than the silhouette.

**Taper, twist and a lattice cage** reach a mesh layer too, as operations on
the form rather than brushes: no centre, no radius, no falloff. There is
deliberately no bend — its map folds distinct points onto the same place past a
gentle angle, so no forward map exists. The cage is the one ZBrush gizmo
deformer that is mesh-only here, because ZBrush and Blender both apply FFD
forward to vertices and an implicit field cannot.

A mesh gesture is **one undo step and reverts exactly**. It has to be recorded
on this side: a vertex displacement is destructive and is not an edit item, so
the document holds nothing to take back — the engine's undo depth is the same
before and after a mesh stroke. The two histories interleave by depth, so one
undo means "the last thing I did" whichever kind of edit that was.

Sculpting **stretches** the triangles it has, and a large grab or a snakehook
stretches them to the extreme. Nothing here retessellates, because that spends
the retopology the import was for; the stretch is reported instead, so a
sculptor learns the mesh wants retopology when it starts wanting it rather than
at export.

## Voxel layers

Nine of the engine's ten sculpting verbs reach a voxel layer through the ordinary
shelf, and two more tools are a different family: **Pintar** colours cells that
are already there and **Apagar** removes them. Painting a grid needs no colour
attribute — a palette always exists, so it creates nothing that was not already
stored, unlike a mesh where the attribute is twelve bytes a vertex and is
refused rather than created.

**Pre-bake repair** is in **Arquivo → Reparar**. A sealed void is invisible
until something needs the model to be solid — a print, a boolean, a
fabrication — so the panel reports what is wrong *before* offering to change
anything, and offers *Preencher vazios* only when there is something to fill.

**Regional refinement** adds a level over a region rather than everywhere,
which is the point of the level stack: block out coarse, then pay for detail
only where the detail goes.

**A grid is drawn, framed and picked by its own routes**, not by the ones the
field uses. The engine is explicit that a voxel layer carries no SDF content,
and three parts of the application had assumed otherwise:

- The viewport builds its surface from the brick cache, which holds the
  document's field. A grid is not in it, so a sculpted voxel layer meshed to
  nothing and rendered as bare ground. It travels the mesh-layer path instead,
  as the **boxes it is** — greedy quads, meshed a chunk at a time.

  The rounded form was the first choice and it does not survive measurement.
  `clay_voxel_mesh_smooth` carries **no vertex normals**, so it draws as a flat
  white silhouette with no form to read; and it is whole-grid with no chunked
  variant, so an edit costs the model. On a 0.01 grid a 3.2 ms dab cost
  **309 ms** to re-mesh, against a 50 ms budget and rising with the sculpt.
  Draining the engine's own dirty-chunk set and meshing only those keys costs
  **3.3 ms** and does not rise — a 24-chunk sculpt re-meshes 7 chunks for a
  dab. The rounded surface is a **conversion away**: cross the grid to SDF,
  which is what that direction is for and where the `Suavização` control lives.
- The polygon counters count what is on screen. They were fed by the brick
  cache alone, so a document whose only layer was a sculpted grid drew
  triangles and reported none of them — "Triângulos 0" over a visible sculpt.
- **Enquadrar tudo** asks the layer for its extent, which the engine answers
  from a layer's *SDF* content. A grid reported none however much was in it, so
  the camera framed a default box. It reports its own extent now.
- A press asks where the surface is, and a ray that meets nothing orbits the
  camera. The raycast marches the field, so a press on a voxel layer orbited
  rather than sculpting. The engine picks a grid directly.

Every one of these passed every test it had, because the tests asked the *grid*
whether it had changed and it always had. `visual_voxel_sculpt.rs` asks the
question a sculptor asks instead — did it appear, does a second stroke move it,
can the pointer find it.

## Crossing between representations

ClayCore carries SDF, voxel and mesh side by side, and the intended workflow
uses more than one: **block out and hard-surface on SDF, free-form sculpt on
voxels, refine on a mesh when the topology is one you want to keep.** Every
representation reaches both of the others, so **six** crossings are offered from
**Arquivo → Converter**, each from the active layer:

| From | To | What it does |
|---|---|---|
| SDF | voxel | Rasterizes the field into cells over the layer's bounds |
| SDF | mesh | Marches the field into triangles — watertight and 2-manifold by construction |
| voxel | SDF | Reads occupancy back, redistanced, as an ordinary operand — one volume item per palette entry, which is what carries the colour |
| voxel | mesh | The grid's exposed faces as merged quads, with the palette colour on the face |
| mesh | voxel | Straight from the triangles in one sampling, so a feature thinner than a cell survives where a field detour loses it, and the vertex colours reach the palette |
| mesh | SDF | Resamples the triangles onto a lattice as a volume item |

The two that end in a mesh are what makes **block out, then sculpt it as a
mesh** a route through the application rather than a description of one. Until
they existed the sixteen mesh brushes could only be reached by importing a file.

The engine meshes a *document*, not a layer — `clay_document_mesh` takes no
layer id and there is no layer-scoped mesher — so the SDF crossing hides the
other SDF layers across the call and puts them back. That is exact rather than
approximate: a hidden layer contributes nothing to the field and showing it
again restores the field exactly, and it is measured. Voxel and mesh layers are
left alone, because neither carries SDF content and neither reaches that mesher.

**The panel states what the crossing costs before it runs**, computed from the
cell size rather than written down, so the figures move as you move the slider:
how far the surface can travel (half a cell), what thickness of feature
vanishes (one cell), how many cells the region holds, and whether sharp edges,
colour and the parametric history survive. A crossing into a mesh states one
more: **the topology is the sampling lattice's and nothing here re-flows it.**
What comes out is dense and uniform, with no edge loop following anything — it
sculpts, and it is the input a retopology pass replaces rather than the output
one produces.

**A layer the viewport has to draw changes the number it watches.** The
carried layers — meshes and grids — are uploaded only when `mesh_revision`
changes, and adding a mesh layer moves no vertex and touches no grid. So a
crossing into a mesh left the number where it was and the layer was never
uploaded: what stayed on screen was the *field* the source layer still
contributed, and removing that source left an empty viewport with the mesh's
62,576 vertices sitting unuploaded. The first stroke moved a vertex, changed
the number the old way, and the mesh appeared. Which layers are carried, and
whether each is shown, are part of the number now.

**A removed layer stops being drawn.** The brick cache holds the evaluated
field brick by brick, and removing a layer used to take it out of the document
while leaving every brick it had contributed to exactly as it was — so the
surface went on being drawn and went on answering a raycast. It looked like the
form you started from sitting under the sculpt and never changing, with the
real result appearing only after a save and a reopen, which builds the cache
from nothing. Measured: a removed sphere still meshed to the same 298,680
triangles through an incremental sync and a full rebuild alike, against 17,160
once its region is re-evaluated. Hiding a layer was always right; both are held
by `a_removed_layer_stops_being_drawn`.

**A crossing produces a new layer and leaves the source alone**, and it is
**not undoable** — the panel says so. A conversion produces no engine undo
entry at all: layer creation and rasterization are not recorded, and a voxel
layer carries no history by construction. Taking a crossing back means removing
the layer it added, which is exactly what the surviving source is for.

Refused rather than approximated: a layer with no bounds and no region, a
resolution whose grid would exceed the memory budget — with the budget named —
an empty source, and a crossing that starts from a different representation.

## Armatures

ZSpheres, with ZBrush's gesture: **drag out of a sphere to grow the next one.**
There is no separate add mode — where the press lands decides what happens,
which is what makes rigging feel like modelling rather than filling in a form.

- Hold **Alt** over a sphere to move it instead. Everything under it comes
  along, in the tree *and* in the surface: lift a shoulder and the arm follows.
- Hold **⌘** to resize; the radius follows the distance from the sphere's
  centre.
- A press on empty space still orbits, so a rig can be turned to look at
  without leaving the mode.
- Mirrored authoring is on by default. One drag makes two limbs, and the
  reflection hangs off the *parent's* reflection, so two arms end up on two
  shoulders. A sphere on the mirror plane is added once, which is what stops a
  spine growing two of everything.
- The gesture plane faces the camera and passes through whatever was grabbed,
  fixed at the press rather than re-derived per sample — a plane that follows
  the pointer drifts, and the sphere slides away from the cursor.
- Skin thickness is a multiplier over the authored radii, so the slider is
  reversible and never rewrites the rig.
- **Esfera negativa** makes a sphere cut into the rig rather than add to it —
  ZBrush's negative ZSphere, said out loud rather than made by pushing one
  inside its parent. The sign belongs to the node, so the membrane along its
  links is not drawn, the carve never sweeps its parent's radius (an eye
  socket does not swallow the head), a negative may carry a limb, and the sign
  survives a save. Only the root refuses, having nothing to cut into.
- The scaffolding — three hoops per sphere, a line per link — draws only while
  rigging, and sits slightly proud of the skin. Flush hoops are invisible,
  because at a joint the skin *is* the sphere.
- Starting a rig gives it a **layer of its own** and hides the others, which is
what ZBrush does by making a ZSphere its own tool: you are not looking at the
model you were sculpting while you build one. The sculpt is hidden rather than
removed — still in the stack, one click from coming back.

**Escultura** opens a rig; `⇧A` toggles editing and `A` the skin preview;
  `Delete` removes the selected sphere and everything under it. The root
  refuses to be removed.

Every rig edit is one undoable action — growing a sphere, inserting a joint,
moving a subtree, resizing, removing, making one negative, changing the skin.
`⌘Z` takes it back and `⇧⌘Z` brings it forward — Ctrl on Windows and Linux, the
platform's primary modifier either way — on the same history as sculpting,
because a sculptor has one undo and does not care which part of the
application produced the thing they want back.

Picking takes the sphere nearest the eye. Rigs overlap constantly — a shoulder
sits inside a torso — and picking the far one makes a chest impossible to grab.

## Viewport

- MatCap shading with five built-in materials, generated rather than shipped as
  assets. Vertex colours modulate the material where a mesh carries them.
- Ambient occlusion, so the surface darkens where it closes in on itself. A
  MatCap is indexed by the view-space normal alone and cannot tell an open
  flank from the bottom of a fold; two passes after the scene read the depth it
  wrote, derive a normal from that, sample a hemisphere around each pixel and
  multiply the result onto the resolved colour. Depth rather than the vertex
  normal because the reference form is about seven triangles per covered pixel,
  where a screen derivative of the normal reports the tessellation instead of
  the shape. Costs 0.08 ms a frame. Requires multisampling, since the pass
  binds the depth buffer as a multisampled texture; a device that falls back to
  one sample draws without it.
- 4x multisampling on the scene, where the device will take it for the surface
  format and falling back to one sample where it will not. The interface is
  drawn into the resolved target afterwards rather than multisampled with it:
  text and panel edges are already laid out on the pixel grid. Measured at
  0.45 ms a frame before and 0.48 ms after, against a 16.7 ms budget — 0.56 ms
  with the occlusion passes above.
- Orbit, pan, zoom, frame-all. Pitch is clamped short of the pole, where the
  view matrix degenerates.
- Four view presets — Perspectiva, Frontal, Lateral, Superior. The orthogonal
  three switch projection and **keep the framing**, so comparing front and side
  does not rezoom. Orbiting away from one stops claiming to be it.
- A ground grid and a symmetry-plane indicator, both excluded from export and
  both measurably dimmer than the sculpt.
- A navigation gizmo in the corner, at a fixed size, sharing the camera's
  rotation and nothing else.
- A brush cursor drawn *in the scene* rather than as a screen circle, so it
  shows the footprint the brush will cover. It clears when the pointer leaves
  the surface rather than hanging at an arbitrary depth.
- Device loss is recovered rather than fatal: the surface, renderer and buffers
  are rebuilt against the same window and nothing authored is lost.
- Level of detail. A model past three extents from the camera drops to the
  brick cache's mips and comes back inside two and a half; the gap between the
  two distances is what stops a resting camera swapping the surface on a
  twitch. A model under 2048 surface bricks is never coarsened — it meshes
  inside a frame anyway. Measured on the reference form: 283,612 triangles down
  to 86,130, with 2.1% of the frame able to tell the difference filling the
  screen and 1.3% at the distance it actually drops at. The interface says
  *detalhe reduzido* while it is drawn. Where no mip has been built yet the
  full surface is drawn instead, and an edit returns to it. See
  [Known-degraded](#known-degraded) for what the coarse surface looks like.

## Interface

The regions from the design: a menu bar, a tool options bar, a left region with
the scene tree, layer stack and sculpt settings, a central viewport, a right
region with material, geometry and brush inspectors, a brush shelf, and a
status area with a memory meter, the active backend and the working unit.

The **accent marks the active brush and nothing else**. Layer selection is
indicated by surface tone and weight; a test asserts the accent's coverage
stays about constant as the active tool changes, so it cannot quietly spread.

Contrast floors are enforced as tests, not intentions: 4.5:1 for text, 3:1 for
indicators that carry state. Where the quiet-until-addressed rule would fall
below a floor, the floor wins.

Every user-facing string is externalised. Brazilian Portuguese is the design's
own language and the default; English is carried alongside, and an untranslated
system tag falls back rather than showing keys.

## Acceleration

Backends are discovered at runtime and ranked per platform:

| Platform | Order |
|---|---|
| macOS | `metal` → `cpu` |
| Linux | `cuda` → `vulkan` → `opencl` → `cpu` |

The CPU backend is always compiled in, so selection never fails for want of a
candidate. Where a backend declines an operation — OpenCL implements neither
raycast nor device meshing — the fallback is **per operation**: the selected
backend stays active for everything it does support, and the fallback is
recorded once rather than once per call.

Backend choice affects speed, never results.

**Refill is routed per batch, and the routing is measured.** A brick refill
goes to the CPU below sixteen bricks whatever the machine — what that avoids is
the fixed cost of a device submission, which is a property of the call rather
than of the hardware. Above it, the first large refill of a session is split
into three slices: a warm-up on the accelerated backend, then one timed slice
each, and every refill after that is timed too, so the routing keeps following
the machine.

This replaced a constant measured on an M-series Mac. It was right there and
wrong elsewhere: on a 24-thread Linux box with an RTX 5060, CUDA is 3.5x
*slower* than the CPU at every batch size from 8 bricks to 7600, and sending
the startup fill to it cost 179 ms against 63 ms. The accelerated backend keeps
a batch unless the CPU beats it by more than a quarter, so a near-tie stays on
the default and only a decisive loss moves the work.

## History

- Undo and redo over the engine's own vocabulary.
- A stroke of any length is **one** history entry, mirrored halves included.
- An edit that changed nothing adds no entry and does not mark the document
  modified. This matters because the engine documents several verbs as
  legitimately able to change nothing — a sub-cell drag, a stamp that misses
  every cell — so a successful call is not evidence that anything happened.
- Changing symmetry costs its own entry, because a layer's mirror is document
  state. It is written only when it actually differs, so a run of strokes at
  one setting costs nothing extra.
- Renaming a layer is one entry too, and the new name is **saved**: it goes
  into the document rather than being kept beside it, so a reopened file shows
  what the layer was called and not what it was created as. A blank name is
  refused — it is what a cleared text field submits — and two voxel layers are
  not allowed to share one, because a grid is reachable only by name and the
  lookup answers with the first match in stack order.

Both are reached from a layer row's own menu, on a right-click, and a rename
also from a double-click on the name — where every layer stack puts that
gesture. The field opens in place, seeded with the name the layer has: Enter
commits, Escape and clicking away abandon. A refusal leaves the field open with
what was typed still in it, so the reason can be acted on rather than arriving
with the work discarded. **Excluir** is disabled with its reason on it for the
last remaining layer, because a document keeps one to sculpt on.

## Geometry in and out

**Importing** asks one real question — reference or clay — because it cannot be
asked afterwards. A *reference* keeps the triangles verbatim on a layer of its
own: a scan, a scale reference, a kit part, geometry that has to leave as what
it came in as. *Clay* resamples into a field and is sculptable from then on.

- OBJ, PLY and FBX. **GLB is export-only** — the engine writes it and does not
  read it — so the import dialog does not offer it and a GLB passed in anyway
  is refused by name.
- A vertex and triangle ceiling is checked against the file's *declared* counts
  before anything is allocated, which is the point: a malformed file can claim
  a billion triangles. The default is well under the engine's own 50M, because
  a ceiling that is never reached is not a ceiling.
- A uniform import scale is baked into the stored geometry, so a unit
  conversion is resolved once rather than approximated by a layer transform.

**Exporting** meshes the field *and* every visible mesh layer — meshing the
field alone would silently leave every imported reference out of the file.
Mesher, cell size and decimation are chosen in the panel, and the watertight
mesher is the default because an export usually leaves for something that will
print or subdivide it.

What the write will give up is said beforehand rather than discovered in the
file: PLY has no texture coordinates, FBX does not carry vertex colour, the
fast mesher is not manifold, dual contouring is experimental upstream, and a
mesh layer without normals costs the whole export its normals — the engine's
concat rule, which drops any attribute that is present on some inputs and
absent on others.

## Documents and sessions

- Save, open, new, save-as, and a **Arquivo** menu carrying all of it plus
  *Abrir recente*, which prunes documents that are no longer there — a menu
  that offers a file and then fails to open it is worse than a shorter menu.
- **Autosave every two minutes**, and only while there is something to lose.
  A marker file written when a session opens and removed when it closes is
  what tells the next run whether the last one crashed; a marker still there
  means the autosave beside it is offered back.
- Recovered work is unsaved work. It does not take the recovery file's path,
  so the next save asks; and it is marked modified, because it is.
- Session state lives in Application Support on macOS and `$XDG_STATE_HOME` on
  Linux. State, not cache: losing it costs work.

## Diagnostics

**Ajuda → Diagnóstico** carries the application version, the engine version,
the vendored engine's git revision, the platform, every registered backend,
the active one and why, the graphics adapter, anything that fell back this
session, and anything that held the interface thread longer than one frame.
One button puts the lot on the clipboard.

The stall list is one line per operation, keeping the worst time and counting
the occurrences — a list with one line per stall is dominated by whatever runs
most often, which is the operation least worth looking at.

**Ajuda → Atribuições** shows the attribution manifest, which is generated
from `cargo metadata` and embedded in the binary rather than shipped beside it.

## Units

One engine unit is a centimetre; lengths read in millimetres. Switching the
display unit is presentation only and changes no geometry — a test states that
directly, in every unit. The status bar's unit readout is the control that
changes it, because that is where a person looks for it.

## Not built yet

Panels cannot be resized or collapsed and shortcuts are fixed. See
[roadmap.md](roadmap.md).

**A brush colour.** Nothing in the application chooses one. The voxel paint and
erase verbs deposit a fixed clay tone, and the SDF combine list leaves `Pintar`
out for the same reason — it is a real engine operation that would colour
nothing, and the brick cache meshes the surface with colours off, so what it
wrote would not be drawn either. Measured at four blend radii, a paint stroke on
a field moves nothing and changes no pixel. The operation stays in the
vocabulary so the mapping onto the engine is complete, and it comes back when
there is a colour to paint with.

## Deliberately absent

**Soft-body dynamics.** The design's *Dinâmica* panel shows gravity, rigidity
and damping. ClayCore has no solver and none is planned, so the panel ships as
voxel size and resolution levels — which is what "Dinâmica: Ligada" means
operationally, closer to ZBrush's Sculptris Pro than to a simulation. The
physics controls are not shipped disabled; they are not shipped.

**Mesh-surface booleans.** A mesh layer can be sculpted — see *Sculpting a
mesh layer* — but it cannot *compose*: it is not an operand of a boolean, a
blend or a deformer belonging to another layer until it is converted, and
paying that conversion quantises the exact vertices and drops the edge loops,
which is precisely what made it worth keeping as a mesh. Convert it if you mean
to subtract from it; the panel says what that costs.

**An alpha stamp on an SDF stroke.** Alphas reach a voxel grid and a mesh, each
by its own route, and both are measured moving the surface. A field takes one as
a deformer appended to an item, and a stroke does not place an item — it hands
the engine a template scaled to each stamp's radius, and the deformer chain hung
off that template does not travel with it. Measured: the same stroke with an
alpha at amplitude 0, 0.05 and 0.25 leaves one surface under both `CLAY_OP_ADD`
and `CLAY_OP_RELIEF`, while the same alpha on a placed item changes the surface
and grades with the amplitude. So the control states the refusal — naming the
stroke rather than the representation, since two of the three take a stamp —
instead of passing an alpha that would be discarded. `claycore`'s
`alpha_deformer` test fails the day the stroke carries the chain.

**A mask gating an operation.** A mask gates *authoring*: a brush does not
deposit where the mask protects. It does not gate an item already in the edit
list, so a subtracting stroke crossing a protected region takes the material
anyway. `clay_item_set_gate` is the entry point that would close that and it is
accepted and inert in 0.39.0 — measured with a mask sampling 1.0 at the cut's own
centre and 65,752 cells painted, at every width and threshold tried, never
refusing. The wrapper is written and matches the documented contract; the
application does not call it, because a call per stroke that does nothing is a
cost with no benefit and a promise the interface could not keep. Both
`mask_gate` tests fail when the engine honours it.

**Windows.** Out of scope for this change.


## Known-degraded

Offered, and not doing what they should. Each has a test that fails when it is
fixed, so this list shrinks by being noticed rather than by being remembered.

ClayCore 0.28.0 emptied most of it: the layer mirror works, `CLAY_OP_ADD`
honours stroke strength, a subset mesh emits its straddlers, and the
bake-and-replace tools no longer corrugate. 0.29.0 took three more — a reopened
document keeps its layer names, visibility and stack order; a saved rig comes
back posable; and the gradient-normal term stopped scaling with the document,
which is what made symmetry affordable.

0.30.0 took three more: the negative ZSphere is now the node's own sign rather
than a separate subtractive sphere; a layer rename is saved rather than kept
beside the document and lost; and a reloaded rig is found by enumerating a
layer's nodes rather than by probing ids and hoping the gaps are short.

What is left is the engine's design or ours. Nothing on this list is waiting on
an engine.

| What | Effect | Issue |
|---|---|---|
| Ruído | Inert. Clamped to zero, because jitter interacts badly with the cache's narrow band | ours, not the engine's |
| The coarse surface, while distant | Faintly speckled, and missing the mips it cannot have. The specks are degenerate triangles shading to a garbage normal — 316 of 86,130 on the reference form — which only shows because level 1 refuses gradient normals and forces face ones. The same specks are on screen during every drag, since the drag shades fast too; there `SurfaceGeometry::refine_within` clears them within a few frames of the pointer stopping, and here nothing can. Separately, a mip needs all eight children evaluated and the cache only evaluates surface bricks, so coarse blocks on the edge of the surface band never get one (70 of 242 here). Splicing full-resolution geometry into those gaps was measured at 0.69% of the frame and not taken — mixing spacings in one surface risks cracks where they meet | the engine's design, and ours |
| Suavizar, Relaxar | Subtle. Relax moves the surface by less than a cell per pass and the cell is 0.02, so they take the high-frequency edge off rather than removing a dent | the engine's design |

