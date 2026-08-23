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

Sixteen fixed-topology verbs reach an imported mesh layer's own vertices, and
all sixteen hold one line above everything else: **topology never changes.** No
polygon is created, split or deleted, so `indices` and quads come out byte for
byte and a model that has just been retopologized can be refined without
spending the retopology.

Twelve of them are tools that already existed — a smooth is a smooth whichever
representation it lands on — and four are new: Argila, Vinco, Pintar and
Borrar. The two colour verbs refuse a mesh carrying no colour attribute rather
than creating one, because twelve bytes a vertex is a real cost to hide behind
a stroke.

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

## Crossing between representations

ClayCore carries SDF, voxel and mesh side by side, and the intended workflow
uses more than one: **block out and hard-surface on SDF, free-form sculpt on
voxels, refine on a mesh when the topology is one you want to keep.** Four
crossings are offered from **Arquivo → Converter**, each from the active layer:

| From | To | What it does |
|---|---|---|
| SDF | voxel | Rasterizes the field into cells over the layer's bounds |
| voxel | SDF | Reads occupancy back, redistanced, as an ordinary operand — one volume item per palette entry, which is what carries the colour |
| mesh | voxel | Straight from the triangles in one sampling, so a feature thinner than a cell survives where a field detour loses it, and the vertex colours reach the palette |
| mesh | SDF | Resamples the triangles onto a lattice as a volume item |

**The panel states what the crossing costs before it runs**, computed from the
cell size rather than written down, so the figures move as you move the slider:
how far the surface can travel (half a cell), what thickness of feature
vanishes (one cell), how many cells the region holds, and whether sharp edges,
colour and the parametric history survive.

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

