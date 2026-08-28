# Features

What works today. Anything not listed here is not built yet — see
[roadmap.md](roadmap.md).

One control is inert, two brushes are subtler than their names suggest, and
the coarse surface speckles while it is distant. None of it is waiting on an
engine: what is left is the engine's design or ours, which is what
*Known-degraded* at the end of this page says of each. They are called out
there rather than quietly omitted, because a tool that is offered and does the
wrong thing is worse than one that is missing.

Every tool names the ClayCore entry point it invokes, so a binding can be
checked against the engine's own documentation without reading the
implementation. A tool with no engine counterpart is not offered.

## Sculpting tools

All twenty are bound and each is covered by a before-and-after capture in
`target/visual/`. Which of the three representations each one reaches is in the
Layers column: eleven have an SDF verb, eleven a voxel one, and seventeen a mesh
one.

| Tool | Engine verb | Layers | What it does |
|---|---|---|---|
| Padrão | `clay_layer_apply_stroke` with relief | all three | Displaces the surface along its normal |
| Inflar | `clay_voxel_sculpt_inflate` / relief | all three | Dilates; a negative amount erodes |
| Suavizar | `clay_item_volume_relax` / `clay_voxel_sculpt_smooth` | all three | Relaxes the surface. Bakes on the field side |
| Mover | `clay_layer_move_surface` | SDF, mesh | Drags the assembled surface. Buds rather than stretches |
| Pinçar | `clay_voxel_sculpt_pinch` | voxel, mesh | Moves surface cells toward the brush centre |
| Raspar | `clay_voxel_sculpt_scrape` | voxel, mesh | Flattens and smooths from one snapshot |
| Planar | `clay_item_volume_flatten_from`, cut-only | SDF, mesh | Planes without filling, which keeps a facet crisp |
| Preencher | `clay_voxel_sculpt_fill_cavities` | voxel | Fills narrow pockets |
| Camada | `clay_layer_apply_stroke`, clamped | all three | A stroke that does not build up on itself |
| Máscara | `clay_mask_apply_stroke` | all three | Freezes a region against every verb. Invert, clear, expand, contract, smooth, bounded complement and extrude are in the Máscaras menu |
| Puxar | swept-sphere chain on a Catmull-Rom curve | SDF, mesh | Pulls a tendril out, tapering to its tip |
| Polir | `clay_item_volume_flatten_from`, cut-only | SDF, mesh | hPolish |
| Relaxar | `clay_item_volume_relax` | SDF, mesh | Relax as a brush |
| Nudge | `clay_voxel_sculpt_smudge` | voxel, mesh | Drags the surface skin, leaving the interior |
| Trim | `clay_cut_create` | SDF | A shape drawn on the frame, cutting through |
| Argila | `clay_mesh_sculptor_stamp` (CLAY) | mesh | Builds up in flat-ish planes, the way clay is added by hand |
| Vinco | `clay_mesh_sculptor_stamp` (CREASE) | mesh | Pinches a sharp ridge or trough along the stroke |
| Pintar | `clay_voxel_paint_brush` / `clay_mesh_sculptor_stamp` (PAINT) | voxel, mesh | Writes colour rather than moving the surface — see *Not built yet* for what it currently has to paint with |
| Borrar | `clay_mesh_sculptor_stamp` (SMEAR) | mesh | Drags the surface sideways without carrying it away |
| Apagar | `clay_voxel_erase_brush` | voxel | Removes cells |

**Trim is not a stroke tool.** Its gesture is a shape drawn on the view frame,
not a drag across the surface, and the interface refuses a stroke for it rather
than doing something adjacent to what the label says.

**Every swatch carries a mark saying what the brush does.** Twenty identical
grey balls told apart by the word under each is a shelf the eye has to read
one by one; ZBrush's is read by shape first. Each mark is a picture of the
effect on a surface — a hump for Standard, a swollen ring for Inflate, ripples
dying to a line for Smooth, a hatch for Mask, a planed-off hump for Scrape, a
tendril for Snake Hook — drawn with one pen at one weight, in the ground's ink
on the lit clay, so the set reads as a set. They are drawn rather than shipped,
which is what lets a test say every brush has a mark of its own and none leaves
its swatch. Hovering a swatch names the brush and says in one sentence what it
does, in the interface's language.

**The shelf holds what the active layer's representation has.** Which tool
reaches which representation is a declared table rather than a rule written per
tool, and the shelf, the availability check and the tests all read it — so the
list you see and the list that works cannot drift apart. A tool with no verb on the active
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

Mesh layers carry the largest vocabulary of the three: the engine's sixteen
fixed-topology brushes, plus Máscara, which writes no vertices and paints the
world-addressed field the sixteen consult. Some of the sixteen arrive as modes
of a tool already on the shelf rather than as rows of their own — one tool
carrying three bindings beats the shelf carrying three tools — which is why the
brush count and the tool count differ. *Sculpting a mesh layer* has the detail.

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

### Symmetry

Symmetry about X, Y and Z reaches all three representations, and by two
different mechanisms because the representations are two different things.

On a **field**, through the layer's mirror — `clay_set_layer_mirror` reflects
the layer's items, so both halves belong to one operation and undo together.
That covers the brushes that *add* an item: Padrão, Inflar, Camada and Puxar.

The five that **bake** — Mover, Suavizar, Relaxar, Planar and Polir — rewrite
the field rather than adding an item, and the mirror cannot reach them.
Measured, a relax with X mirrored took the surface under the stroke from 1.1467
to 1.1409 and left its reflection at **1.1467 exactly**. Their strokes are
reflected instead, the way a mesh's and a grid's are.

All six of those used to bypass the symmetry argument entirely, and the fault
ran both ways: never *setting* the mirror, they inherited whatever it was last
told, and the starting form turns X on — so a snakehook with symmetry switched
**off** came out on both sides at 1.4625. Every SDF stroke points the mirror
now.

On a **mesh** and on a **grid**, by mirroring the *stroke* and applying it
again. There is nothing else to reach for: the layer mirror reflects a layer's
items, and a mesh has vertices while a grid has cells. That is also what both
references do in the same position. Measured in Blender 5.2, one Draw dab on a
64×32 sphere:

| Symmetry | +x | −x | +y | −y | Max displacement |
|---|---|---|---|---|---|
| none | 82 | 0 | 78 | 0 | 0.18306 |
| X | 82 | 82 | 156 | 0 | 0.18306 |
| X and Y | 161 | 161 | 156 | 156 | 0.16893 |

One dab per reflection at full strength, and **two axes give four dabs** rather
than two — the full subset lattice, which is what a sculptor means by
"symmetric in x and y": the four quadrants, not the two halves twice. Three
axes give eight. Every reflection goes into the same set of deltas, so a
symmetric stroke is one undo however many copies of it the axes called for.

A reflection turns a **direction** over as well as a position. Forgetting that
is the bug that makes a mirrored Grab send both sides the same way in world
space — one out of the model and one into it — instead of moving them as a
pair.

This was inert on both. `apply_stroke` takes the enabled axes and the mesh and
voxel arms of its dispatch dropped them before the stroke functions ever saw
them, so every symmetry button in the interface did nothing on anything but a
field.

**On counting**: our mesh comes from marching cubes, whose vertex density is
not the same on both sides of a plane — a lone dab moves 497 vertices at one
place and 272 at its mirror, and a lone dab *at* that mirror moves 272 too, so
the mirroring is exact and the difference is the tessellation's. Blender's UV
sphere is symmetric by construction, hence its 82/82. What a sculptor means by
symmetry is that the *form* comes out symmetric, so that is what the tests
measure.

### A tube along a curve

**Dinâmica → Tubo por curva** places a curve: click to put a control point
down, drag one to move it, Del removes the selected ones. What makes it
different from a brush is not the shape it leaves but that it can be **gone
back to** — a stroke is over when the pointer comes up, and a curve is a set of
points that stay where they were put. Nomad calls it a Tube, 3DCoat a spline.

Everything under it was already in the engine: `CLAY_PRIM_SWEPT` carries a
profile along a guide, `clay_item_add_loft_profile` supplies the profiles,
`clay_item_set_curve_points` types each point, and
`clay_layer_set_stroke_points` edits a placed guide undoably. Editing
**replaces** the sweep rather than adding another, the same way a snakehook
gesture grows one tendril — otherwise dragging a control point would leave a
tube behind on every move.

| Control | What it does |
|---|---|
| Espessura | Thickness at the selected points, or at all of them where nothing is picked |
| Junção | **Cantos** straight, **Pelos pontos** Catmull-Rom through them, **Arredondado** a B-spline that rounds corners off |
| Perfil | Círculo, Quadrado, Hexágono, Triângulo |
| Aplicar | Leaves the swept form and takes the curve down |

**Two primitives, because they do different things.** A **round** tube is a
swept-sphere chain — the snakehook's primitive — which takes a radius *per
point* and so tapers along its whole length. Any other section is the swept
primitive, and that one **ignores the guide's per-point radius entirely**:
measured, the same guide swept with radii of 0.05, 0.15 and 0.4 reached 2.901
every time, the unit profile's own size. Its thickness comes from the profile
parameters, so a sectioned tube takes the *first* point's thickness at one end
and the *last* point's at the other and interpolates between — a taper, but not
a radius per point.

**A curve is an item, so the layer's mirror reflects it.** A tube placed on one
side of a mirrored layer appears on both, which is what symmetry is for — but a
tube laid *across* the plane is folded onto itself and reads as symmetric
whatever its radii do.

### Pulling a tendril

**Puxar** authors a curve — a chain of spheres swept along the path, tapering
toward the tip — rather than stamping along it. Two things make it read as one
tendril rather than a string of beads, and both were wrong at first:

- **One gesture grows one curve.** A drag arrives in segments, and a segment
  that authored its own item restarted the taper from full width every time.
  The gesture holds the curve it is pulling and *replaces* its points, so the
  tendril is the length of the whole drag. Measured on a curving pull, the
  thickness along it wobbled by **0.210** that way against **0.122** now — and
  a single tapering curve wobbles 0.137 from the taper alone.
- **Its points are joined by a spline.** A stroke's points are hard corners by
  default, which is right for a chain authored point by point and wrong for a
  path a pointer traced: every sample became a kink and the swept sphere
  bulged at each one. Catmull-Rom passes *through* the points, so the tendril
  is the path the pointer took. A straight drag hides this entirely; a curving
  one is where it shows.

The curve is held only while a gesture is open, so the next pull is its own
tendril rather than a continuation of the last.

## Masking

**M** starts painting a mask and **M** again puts the tool you were using back
in your hand — the key Blender's sculpt mode uses, and a toggle because
freezing a region is a detour from what is being sculpted rather than a mode to
live in. Choosing a tool from the shelf ends the detour, so a later **M** starts
a fresh one rather than rewinding past the choice. The material cycle, which
held `M`, is **Shift+M**.

The frozen region is **drawn**: masked clay reads as a dark neutral over the
shading, at roughly three quarters strength so the form underneath stays
legible. This is the same as Blender and it is not decoration — masking
protects the surface almost completely (measured, 1.0005 against 1.1400 for the
same stroke unmasked), so a sculptor who cannot see the mask cannot tell a
protected surface from a broken tool.

A mask belongs to **no representation**. It is a world-addressed field the verbs
consult, so it is painted the same way and honoured the same way on a field, a
grid and a mesh. That was not true before: on a grid the tool fell through to
the depositing arm and *added clay* where the sculptor asked to freeze a region,
and on a mesh it was refused outright even though a mesh stroke had been handing
the mask to the engine all along. `masking.rs` holds both.

### What the Máscaras menu does

Every entry acts on the mask itself rather than through it. The amounts live in
the **MÁSCARA** section of the inspector, which appears once a mask exists; the
menu spells out what it would apply, because the same entry now does a
different amount of work depending on the panel.

| Entry | What it does | Amount |
|---|---|---|
| Inverter | Frees what was frozen and freezes what was free | — |
| Expandir | Grows the frozen region, in cells | Passos |
| Contrair | Shrinks it | Passos |
| Suavizar máscara | Softens the region toward its neighbourhood | Passos |
| Complemento delimitado | Inverts *inside the region the mask already covers* | — |
| Limpar | Unfreezes everything | — |
| Extrudar | Pulls the frozen patch off as a wall in its own layer | Espessura, Arredondar, Suavizar borda, and the side |

**Inverter reaches only where the mask has been.** A mask is a sparse field and
inverting fills the blocks it has *allocated*, not the universe — the far side
of the model stays free. That is what makes the operation finite, and it is why
**Complemento delimitado** is a separate entry: bounded by what the mask
already covers, it is the "everything except this, here" a sculptor usually
means.

**Extrudar reads the mask rather than consuming it**, so an extrusion you do
not like can be thrown away without painting the mask again, and the patch
arrives as its own layer rather than as an edit to the one it came from.
Measured on a unit sphere with a 0.2 wall: **Para fora** takes the surface to
1.16, **Para dentro** leaves the outside at 1.000 and builds inward, and
**Centrado** reaches 1.1015 — half the thickness above the surface, which is
what half each way means.

**Extrudar needs something to sample.** `clay_document_mask_extrude` samples a
*layer's field*, and a grid has a verb of its own that works from its cells
without a conversion — so an SDF layer and a voxel layer both extrude, and both
produce an SDF row, so the operation means one thing whatever it was run on. A
**mesh layer has neither**: the entry is greyed there, and the reason names the
way round, which is a crossing to SDF. It was offered on all three and worked
on one, with the refusal going into a notice nothing displayed — a click that
did nothing at all.

Three of these took an amount the interface could not set: `Expandir`,
`Contrair` and `Suavizar máscara` were dispatched with a hard-coded 1, and an
extrusion with every default it was born with, so its thickness, rounding and
edge smoothing were unreachable and every wall the application could build was
0.08 thick. `mask_operations.rs` measures each entry and each amount.

### Which brushes have a sign

Holding the invert key turns a brush over where it has an opposite, and does
nothing where it has not. That is a rule rather than a gap:

| Brush | Held | Why |
|---|---|---|
| Padrão, Inflar, Camada | takes material away | depositing has an opposite |
| Planar, Polir | **fills instead of cutting** | planing is cut-only so it does not fill the dents it reveals; the other half is fill-only, which the engine has had a mode for all along |
| Suavizar, Relaxar | nothing | an inverted smooth is not a thing either reference offers, and sharpening is a different verb rather than a smooth turned over |
| Mover, Puxar | nothing | a drag's direction *is* its sign; inverting it is dragging the other way |

Measured on a sphere with a bump and a dent beside it: upright, planing takes
the bump from 1.1150 to 1.1145 and leaves the hollow at 0.8923; held, it fills
the hollow to 0.9004 and leaves the bump exactly where it was.

### Which voxel brushes have a sign

Three of the engine's voxel verbs come in **documented pairs**, and only one
half of each was ever asked for:

| Brush | Held | The engine's own words |
|---|---|---|
| Padrão, Camada | erases | — |
| **Inflar** | **erodes** | *"amount > 0 dilates, < 0 erodes"* — the binding passed a hard `1` |
| **Pinçar** | **spreads** | `clay_voxel_sculpt_magnify`, *"pinch's inverse, sharing its walk so the two cannot drift apart"* |
| **Apagar** | **deposits** | the one tool whose upright verb is the removal |
| Suavizar, Relaxar | nothing | a majority filter has no sign |
| Nudge | nothing | a smudge's direction *is* its sign |
| Raspar | nothing | see below |

**Raspar looked like a pair and is not.** Turning the scrape's normal over
moves 2580 indices to 2568 — both directions remove, because the normal there
is a fixed up-vector rather than the surface's own, so flipping it scrapes some
other face rather than reversing the verb. Left unbound: a guess dressed as a
feature is worse than an honest absence.

**Pintar is inert on a grid, and says so.** It colours cells that are already
there, and the palette holds one entry because nothing in the application
chooses a brush colour — so it paints cells the colour they already are and
reports `changed: false`. That is the gap under *Not built yet* rather than a
broken binding.

### Held keys

Two keys change what the stroke in your hand does, for the length of that
stroke only. Both are read **at the press** and held for the gesture: a key
caught or let go mid-drag would change the verb under the sculptor's hand, and
neither ZBrush nor Blender does that. The shelf never moves, so letting go
returns to the tool that is selected without having to re-pick it.

| Key | Effect |
|---|---|
| **Shift** | Smooth instead, whatever tool is selected |
| **Ctrl** | Take material away rather than put it there |

**Ctrl and not Alt**, which is what ZBrush spells it. Alt already forces the
drag to orbit — ZBrush's own rule, and the one that leaves a trackpad with no
second button able to turn the model — and while rigging it means "move this
sphere". Blender spells invert Ctrl, and Ctrl is free here during a stroke.

Inverting means a different thing on each representation, and each one is what
that representation has:

- On a **field** the combine operation is turned over: Add becomes Subtract,
  Emboss becomes Engrave, Relief becomes Incise. An operation with no opposite
  — Intersect, Replace, a seam — is left as it is rather than quietly becoming
  some other verb.
- On a **mesh** the brush descriptor's strength is negated, which is signed for
  every verb that has a sign: Padrão digs, Inflar deflates, Vinco cuts. Note
  that this is the *descriptor's* strength and not the stroke preset's — the
  preset's is contracted to `[0, 1]` and the resolver drops any stamp whose
  strength is not positive, so a negative preset strength is not a dig but
  nothing at all. Measured on a unit sphere, a sweep raises the surface to
  1.054 upright and lowers it to 0.945 held.
- On a **grid** occupancy is binary, so there is no sign to turn: the opposite
  of putting a cell there is removing it, which is `Apagar`'s verb.

Holding both is a smooth. Sharpening is a different verb rather than a smooth
turned over, and neither reference offers an inverted smooth.

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
| Inflar | 5.04x | all three | 1.00x |
| Pinçar | 9.41x | voxel, mesh | 1.00x |
| Vinco | 3.71x | 1.34x | 1.00x |
| Padrão | 1.11x | all three | 1.00x |

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

## The deformation cage

ZBrush spells it the Gizmo Lattice, Blender the Lattice modifier, Maya an FFD.
All three show the same thing and so does this: a box of control points around
the model, dragged directly in the viewport, with the form following.

**Dinâmica → Gaiola de deformação** puts one up, sized to what the layer
actually contains and standing a little proud of it — a corner point buried in
the clay is not a handle. Drag a point; **Deformar** bends the layer through
the cage and takes the cage down. Nothing moves until then: a cage is worked
in, across many pulls, and a form that lurched on every drag could not be aimed.

The whole cage is **one undo** however many points were dragged, because that
is the unit a sculptor thinks in — they bent the form once. An untouched cage
is exactly the identity and applying one is a no-op rather than a pass over
every vertex to move them all by zero.

**On a mesh the form follows while you drag.** The forward route deforms
vertices the sculptor already has, so showing the bend is one pass and taking
it back is one more — measured at **11.2 ms** a frame on 62,576 vertices. Every
drag replaces the last rather than adding to it, so the preview never
compounds and the whole gesture is still one undo. Abandoning the cage takes
the preview back with it.

**On a field the drawn surface follows too**, by a different route. Applying a
field cage writes a deformer into the document as an undoable edit and refills
the layer's whole brick region — **68.8 ms** for one apply on the starting form
— which is not a thing to do on every pointer move. So the preview moves the
vertices the viewport already holds, by the warp the engine supplies
(`clay_mesh_lattice_displacement`, which exists for exactly this), and no
lattice arithmetic is written twice.

That warp is the **forward** map where the field's own deformer is the inverse
one. They are not the same map, and the size of the difference is the whole
question. Measured against the engine's own result on a cage spanning ±1.1:

| Drag | Preview against the engine |
|---|---|
| 0.05, 0.10, 0.25 | **0.6%** of the drag |
| 0.50 | 16% of the drag |

So the preview tracks closely for ordinary work and drifts on a drag most of
the way across the box. That is a preview's error budget rather than an edit's:
what lands on **Deformar** is the engine's, computed the engine's way, and the
surface settles onto it. Taking the cage down puts the drawn surface back — the
preview moved vertices the document knows nothing about, so nothing else ever
would.

A press on a control point takes the primary button before the surface does.
That is not an ordering detail: a control point sits *outside* the form, so a
press on a corner handle would otherwise find the clay behind it and start a
stroke on the layer the cage is there to bend.

**Two routes, two ceilings**, and the difference is the mechanism rather than a
limit someone picked:

| Layer | Route | Points per axis | How |
|---|---|---|---|
| Mesh | `clay_mesh_sculptor_lattice` | 2–**32** | Forward. Each vertex evaluated once; nothing inverts, iterates or approximates |
| SDF | `clay_layer_lattice_gizmo` | 2–**4** | An inverse point map, resolved into one lattice deformer per item and evaluated at every sample |
| Voxel | — | — | Neither a forward vertex pass nor a deformer stack. The entry is greyed, naming the crossing |

Measured on a unit sphere: pulling the four top corners of a 2×2×2 cage up by
0.5 takes the mesh's highest point from 0.9999 to 1.4772 — a corner control
point is interpolated, so dragging one moves that corner of the box exactly.
The same pull forward on a 4×4×4 field cage takes its reach from 1.000 to
1.5777.

### A cage is a mode

While one is up the layer is being *deformed*, and three things follow from
that:

- **The brushes are off.** A press that misses a control point orbits rather
  than sculpting. It used to fall through to the brush, so a slip while aiming
  sculpted the very form the cage was there to bend — and the strokes it left
  made the next control point harder to hit. Orbiting rather than nothing, so
  the cage can still be turned to look at from behind without being taken down.
- **The form is drawn through.** Half the control points are behind it, and a
  solid surface hides exactly the handles that need reaching. Blender's X-ray
  and ZBrush's Ghost do the same thing for the same reason. Seen through, not
  turned off: the form stays readable as a form.
- **Handles keep their size.** They are sized from the box the cage was *built*
  with, not from where its points are now. Sized from the current extent — as
  they were at first — hauling one corner out inflated every other handle, so
  the targets a sculptor was aiming at swelled under the pointer as they
  worked.

### The manipulator

A click selects one control point; **Shift-click** adds or removes one without
disturbing the rest. That is what the manipulator exists for — dragging points
one at a time needs no widget, and turning a whole face of the cage cannot be
done without one.

It sits on the **middle of the selection**, not on the last point picked, so
adding a point moves the widget to where the selection is. One widget with
three modes — **Mover**, **Girar**, **Escalar**, chosen in the GAIOLA section —
which is what ZBrush and Maya both settled on: the hand stays where it is and
the mode is what changes.

Shapes rather than colours alone carry the meaning: an arrow slides, a ring
turns, a box scales. A person reaching for a handle is not reading a legend,
and the three axis colours are the one part of this a colour-blind sculptor
cannot use.

- An **axis** drag is constrained to that axis. Pulling the green arrow means
  "up", not "up and a little sideways because my hand drifted".
- The **centre** moves freely in the view plane, and scales uniformly. Rotation
  has no centre handle: turning about the axis facing the eye is what the outer
  ring is for, and a filled centre meaning the same thing is one more thing to
  hit by accident.
- The **outer ring** turns the selection in the plane of the screen — ZBrush's
  outermost ring, and the one a sculptor reaches for most. The three axis rings
  turn it in the *world's* frame; this one turns it in the frame it is being
  looked at from. It is the only handle whose axis is not a world axis, so a
  drag carries the direction the camera faced **when the press landed**: an
  axis re-read each frame would twist the selection under a hand that had not
  moved. It sits outside the three at 1.28× their radius — among them it would
  be a fourth thing to tell apart at the same distance from the pivot — and it
  is tested last, so a press where it crosses an axis ring goes to the axis.
  The outer one is the easy target everywhere else and should not steal the
  hard ones.
- **Turning and scaling need two points or more.** They act about the middle
  of the selection, and one point's middle is itself — so on a selection of one
  they are exactly no movement, however the drag is made. The two modes are
  disabled with the reason on them rather than drawn live and inert, which is
  how they were: the rings appeared, the drag ran, and nothing moved. Moving is
  not affected; it needs no pivot.
- **A ring can be grabbed anywhere along it.** It is hit-tested as a string of
  spheres, and sixteen of them was a number picked rather than derived — at the
  manipulator's own proportions they do not touch, so about a fifth of every
  axis ring and a third of the outer one could be pressed with nothing under
  the press. The count comes from the ring's circumference and the grab radius
  now, and a test walks a thousand points around the ring rather than checking
  at the samples.
- **Ctrl snaps a turn to 15°.** Twenty-four increments to the circle, which
  divides the angles a sculptor actually reaches for — 30, 45, 60, 90 — where a
  rounder-looking 10 does not. Rounded to the *nearest* rather than downward, so
  the handle stays under the pointer across a boundary instead of lagging half
  an increment behind it, and read **per drag rather than per gesture**: the
  modifier can be taken up part-way through a turn to land it on a round number,
  which is how Blender's works and what a hand actually does. It is angle
  snapping only — a move that snapped to a grid nobody asked for would be a
  surprise.
- A **scale never passes through zero**, either way. A drag that overshot the
  pivot would turn the form inside out with no way back but undo.
- A drag is resolved **from its anchor every frame** rather than accumulated.
  Transforming what the last frame produced compounds a rotation into a spiral
  and a scale into a runaway.

**A slide and a turn run on opposite planes**, which is the part that is easy
to get wrong and was wrong here for as long as the manipulator existed. A ring
lies in the plane *perpendicular* to what it turns about, and that is where the
angle is measured — dragged on a plane containing the axis instead, the
pointer's travel has no component in the plane being measured and the turn
comes out at exactly zero however far the hand moves. Two of the three rings
did nothing at all; only the one whose axis pointed at the camera worked. The
same line put the axis *facing* the eye on a plane perpendicular to itself,
which sets the anchor's component along it to zero — and since a scale divides
by that, that handle went dead too. The plane is chosen by the mode now, and
`drag_plane` is a pure function with the two rules stated separately.

An axis drag runs on the plane containing that axis and most nearly facing the
eye — not on a plane facing the camera outright, which would make an axis
pointing at the viewer unmovable: the pointer could travel a long way and its
projection onto the axis would barely change.

Press order in the viewport is **manipulator, then control points, then the
surface**. The manipulator is drawn over the cage and sits on the selection, and
the cage sits outside the form; without that order a press on the green arrow
finds a control point behind it, and a press on a corner handle finds the clay.

### Boxes or a surface

A grid *is* boxes. Whether it should **look** like boxes is a separate
question, and the engine answers it plainly: the boxy picture is "correct for
hard-surface voxel work and for export, and the wrong picture of an organic
sculpt". It ships a mesher for each and keeps the choice an argument rather
than grid state, "so two hosts sharing a document cannot disagree about what it
looks like and one host can show both pictures of one sculpt without mutating
it".

**The smooth surface is the default.** A sculptor is shaping a form, not a
lattice, and the cells a grid is stored in are a fact about the storage —
showing them by default would make a voxel layer the odd one out for a reason
that belongs to how it is kept rather than to what it is. **Exibir voxels como**
in the inspector switches to the boxes when seeing the cells is what is wanted.
Nothing either choice does touches a cell, enters the history or marks the
document modified.

| | vertices | triangles | mesh time | normals |
|---|---|---|---|---|
| Voxels (greedy) | 6828 | 3414 | 1.5 ms | carried |
| **Suave, blur 0** (default) | 2221 | 4980 | 16.8 ms | **computed here** |
| Suave, blur 1 | 992 | 1992 | 19.0 ms | **computed here** |

The smooth surface is also the *smaller* mesh — a third of the vertices at
0.05, and 11,390 against 21,268 at 0.02, because a box mesh spends vertices on
corners the form does not have.

Two facts shape how it is wired, and both are the engine's rather than
preferences:

- **The smooth mesh carries no normals.** Colour blends across a smooth surface
  — a vertex sits between up to eight voxels and averages the occupied ones,
  because there is no facet to hold one palette entry — but a normal is the
  host's to work out. Without them the surface renders as a flat silhouette,
  which is what the first attempt at this looked like. They are computed
  area-weighted on the way through.
- **It cannot be meshed a chunk at a time.** `clay_voxel_mesh_chunks` is the
  greedy mesher alone, because greedy quads are axis-aligned and clamp to a
  chunk boundary exactly while surface nets place a vertex from a cell's
  *neighbourhood* and would tear. So it is rebuilt **whole**, guarded on the
  grid's own change count: a frame in which nothing moved costs one comparison,
  and one in which something did costs a re-mesh. Measured, 17.3 ms at a 0.05
  voxel size, 18.0 at 0.03 and 20.6 at 0.02 — flat enough in the size of the
  grid to sit on the frame path rather than waiting for a gesture to settle,
  which a form that lagged the brush by a whole stroke would have to.

**Suavização** is the engine's `blur`, in passes of a 3×3×3 box over occupancy,
and its trade is real in both directions. At **0** nothing is filtered and
nothing can be lost, but the surface still *terraces* — every crossing over
binary occupancy interpolates to the same midpoint, so corners round and steps
remain. At **1** it reads as clay, and an isolated voxel sits near 0.3
occupancy, under the isolevel, and is gone; thin features go the same way. The
default is 0, and the interface says so where it is not: "a default that
silently deletes a sculptor's detail is the wrong default however good it
looks."

**This is surface nets, not dual contouring.** A vertex sits at the *centroid*
of its cell's edge crossings, which is what smooths — so a corner rounds.
Dual contouring fits the vertex by least squares to hermite data and keeps a
sharp corner sharp. Preserving them would be a change to the engine rather
than to this application.

## Placing shapes, and the booleans on them

*Escultura → Formas* offers fourteen shapes — box, sphere, cylinder, cone,
torus, capsule, ellipsoid, pyramid, rounded box, frame, rounded cylinder, hex
and tri prisms, octahedron — each with the numbers it is actually measured by,
which are different numbers for different shapes. **Colocar** puts one where
the pointer is on the surface, or where the camera is looking when the pointer
is off it. The two the engine calls unbounded — a plane and an infinite
cylinder — are not offered: neither has an extent for a manipulator to sit on
or a bound for the cache to work from.

**A placed shape stays live.** It is an item in the layer's ordered list, so
select it a week later, move it, and the boolean follows: the hole is where the
cylinder now is. Its operation is a property of *it* rather than of the gesture
that made it, so a subtraction can become a groove without replacing anything,
and its shape can be exchanged without losing where it stands. The same
thirteen combine operations a stroke has, on the same terms — including the
seven whose slider cannot reach zero, because zero there is not a hard join but
no operation at all.

Clicking a placed object in the viewport selects it, and the engine attributes
a hit to the item whose field carved the surface — so **clicking the wall of a
hole selects the cylinder that cut it**, which is what clicking there means. A
press only looks for an object while the panel is open or one is already
selected: a sculptor mid-stroke must not have a press become a selection
because a cylinder happens to sit under the brush.

A selected object is **outlined** in the viewport. A subtracting object is
behind the surface — what you see of a bore is the hole — and without the box
there is nothing to aim but a manipulator over a cavity.

The starting form is a placed sphere and is listed as one. It always was;
nothing but the absence of a selection model made it special. It can be
selected, resized and deleted like anything else.

### The manipulator on an object

The same widget the cage has, on a placed object, a whole layer, an imported
mesh, or a curve's control points. All the rules are the cage's: it sits on the
middle of what it acts on, an axis handle constrains the drag, a wandering hand
lands where it settles, and a scale never passes through zero.

**Scale is uniform, and the widget says so.** Every transform in the engine's
interface takes one scale factor and not three, so scale mode offers the centre
alone on an object, a layer or a mesh — there are no axis boxes, because three
handles for one number is either two that do nothing or three that lie. A cage
keeps all three: it scales its own control points and carries no engine
transform. Use the cage when you mean to stretch along one axis.

A whole drag is **one undo step**, however many frames it took.

**The widget is drawn over the clay, not in it.** A manipulator sits on the
middle of what it moves, and the middle of a placed sphere is inside the
sphere; depth-tested, it was three arrow tips poking out of the form and
nothing to grab, and on a small object inside a large one it was nothing at
all. The cage, the curve's control polygon, an object's outline and the
manipulator are all scaffolding around the clay, and scaffolding the clay hides
is not scaffolding — so the overlay reads no depth, and every handle is where
the hand expects it whichever side of the surface it is on. The strokes are laid
down three deep, stepped across themselves in the screen plane, so a handle is
a handle and not a one-pixel hairline over a shaded form.

**It is the same size to the hand at every zoom.** The arms are a share of the
distance to the camera's target rather than a length in the scene — the rule
ZBrush, Maya and Blender all follow. A fixed length left the screen when the
sculptor zoomed in and shrank to a speck when they zoomed out. The drawing and
the hit test read one function, so the handle drawn and the handle grabbed
cannot come apart.

A sculpting stroke is not a target. A stroke is a gesture that has finished,
and picking one back up is a different question — which of its samples is being
moved — that moving all of them silently would answer wrongly. Clicking one
says so on the options bar rather than doing nothing, and the press stays the
brush's: a press on the clay is a stroke, and taking that away to explain
something would be the worse error.

### When the clay is behind your hand

A live boolean is re-evaluated on every frame of a drag. Measured on the
reference scene, one frame costs about 21 ms against a 16.7 ms budget, and on a
heavier form it costs more. Where a frame overruns, the object goes on moving
at the speed of the hand and the surface catches up **once**, when the pointer
comes up — the same answer the region-based brushes already give, and for the
same reason.

One of the thirteen operations is dearer than the rest to drag. The engine
drops a node's finite influence bound for a non-local operation anywhere in the
subtree, so an object set to **Interseção** dirties the whole layer on every
frame while the same object subtracting dirties its own box. Measured on the
same object and the same scene: 21.3 ms a frame subtracting against 49.1 ms
intersecting — `object.drag_frame` and `object.drag_frame_intersect` in
`benchmarks/baseline-linux-x86_64.json`, better than twice the cost. It is not
visible from the interface, which is why it is worth saying here.

### A model as an operand

An imported mesh appears in the shapes picker, under the shapes and separated
from them, because placing one is a **crossing** and costs something the shapes
above do not. Choosing one states what it costs before anything happens — the
surface movement, the feature size that vanishes, the cell count, the sharp
edges lost — which are the conversion panel's own figures for the same crossing
at the same resolution.

The mesh layer is left exactly as it was and stays sculptable with the sixteen
fixed-topology brushes. What is placed is a sampled copy, and it behaves like
any other placed object from then on.

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

### Zooming

The wheel zooms **at what is under the pointer** and stops a little short of
it, which is Blender's behaviour and two things rather than one:

- **It stops against the clay.** The distance used to be clamped to an
  arbitrary floor rather than to anything in the scene, so a few notches too
  many put the eye through the surface and the sculpt turned inside out. The
  camera now comes as close as it likes and never through: what must stay
  between it and the surface is a *fraction* of the gap — so the standoff is
  the same on screen whatever scale the sculpt is at, where a fixed one would
  be a mile on a thumbnail and nothing on a bust — and never less than a little
  in front of the near plane, since a surface closer than that is clipped away,
  which looks exactly like having gone through it.
- **The pivot follows part of the way**, so the point under the pointer drifts
  toward the middle and the next orbit turns around what you were looking at.
  Partial rather than complete: snapping the pivot onto the surface would swing
  the view on every notch.
- **A notch is about seven per cent**, and it is a *factor* rather than a
  fraction taken off one. Both matter. The rate is fine enough to creep up on a
  detail and still compounds — ten notches halve the distance — and a factor
  cannot cross zero, where the subtracted form asked for a negative distance
  past ten notches in one frame and only a clamp caught it. A factor is also
  symmetric: a notch in and a notch out land exactly where they started, where
  the old form left the camera slightly nearer each time a wheel was jiggled.
- **The wheel is measured in notches, not points.** egui reports scrolling in
  points and one notch is forty of them — a number chosen for scrolling a
  document. Handed to the camera raw, as it was, a single notch asked for forty
  notches of movement: inward a negative distance, outward five times further
  away in one click. That is what "the zoom jumps are too big" was. The
  conversion divides by egui's own figure rather than a hardcoded forty, so a
  trackpad's smaller deltas come through as the fraction of a notch they
  actually are and a two-finger drag glides instead of stepping.

Two things it does *not* do. Zooming **out** is never held back — the standoff
limits coming in and nothing else. And with the pointer over empty space it is
the plain multiplicative zoom, because there is nothing there to stop at and a
wheel that refused to move would read as broken.

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

### Reference images

**Vista → Imagens de referência…** (View → Reference images) opens a panel with
a row for each of the three orthogonal planes. Load a PNG onto a plane and it
hangs behind the origin on that plane, to sculpt from.

- One picture a plane, each with its own placement: shown or not, opacity,
  height, an offset across and up within the plane, and how far back it sits.
  **Width follows the image's own proportions** — a reference is never
  squashed, whichever plane it is on.
- **The clay is always in front**, from every angle. The quad sits behind the
  origin, but that alone stops holding once the camera swings past its plane,
  so references are drawn first and write no depth. A guide that occludes the
  form it is guiding has stopped being a guide.
- **PNG and JPEG** — drawings and cut-outs in one, photographs in the other,
  which between them is what a reference folder actually holds. Anything else
  is refused by name rather than by a decoder message about chunk headers.
- The file's own alpha is kept and multiplied by the opacity, so a cut-out
  placed at half opacity is still a cut-out. Colour is kept — unlike an alpha
  stamp, which is flattened to one scalar a pixel — palette PNGs are expanded,
  since an index read as a colour is not a picture of anything, and a greyscale
  photograph is spread across three channels rather than sampled as red.
- **A photograph taken sideways is turned the right way up.** A phone stores
  the sensor's own orientation and an EXIF tag saying how to turn it; ignored,
  the reference arrives sideways, and a sideways reference is not a reference
  but a puzzle. All eight orientations are applied, once, to the pixels — so
  the placement a sculptor sets is about the reference and not about how the
  camera was held. Only that one tag is read: the rest of an EXIF block is
  someone's camera model, their lens and often where they were standing.
- **The model's own opacity** is a slider at the top of the same panel. The
  clay turns translucent so the reference shows through it — Blender spells
  this X-ray and ZBrush spells it Ghost, and both are a switch; this is a dial,
  because tracing a silhouette against a photograph wants a different amount
  from reaching a deformation cage's control points. It cannot be taken to
  zero: a surface faded to nothing loses the form, the brush cursor's footprint
  on it and any way of telling a stroke landed — turning the layer off is what
  turning the layer off is for. Raising a cage still imposes its own ceiling,
  and lowering the cage does not silently make a deliberately faint surface
  solid again. It opens solid each run and is not remembered.
- **Not part of the document.** A reference is what the sculptor is working
  *from*, not what they are making, and a document carrying someone else's
  photograph is a document that cannot be shared. Loading, moving, fading or
  clearing one does not mark the document modified. The paths and placements
  are remembered with the session instead — see
  [Documents and sessions](#documents-and-sessions).

## Language

**Vista → Idioma** (View → Language) chooses between three complete
translations: **Português (Brasil)**, **English (US)** and **Español
(Latinoamérica)**. Each is named in itself, which is the one rule a language
menu has — a reader who cannot read the current interface can still find their
own. The choice is written to the session directory, so it survives a restart.

The interface **opens in English**. That is not the design's own language and
it is deliberate: it has to open in something a first-time reader can make
sense of. A system language still wins over that default on a first run —
`LC_ALL`, `LC_MESSAGES` or `LANG`, matched by language rather than region, so
`pt_PT`, `en_GB` and `es_ES` each find their translation. A choice already made
wins over both.

All three translations shipped from the beginning **with no way to choose
between them**: the locale came from `Locale::default()` at startup and was
never asked about again, so `Locale::from_tag` — written for exactly this — was
called by nothing.

**The brush names are translated.** All twenty, on all three representations —
the shelf and the status bar's last action both read them from the interface's
own table. They were `ToolKind::label()`, the domain's own Portuguese, shown
whatever the language was. `ToolKind::label` keeps that Portuguese for the
places that are not the interface: history entries, engine refusals and the
diagnostics report, none of which has a language.

One thing worth naming, because it reads as correct to anyone checking a single
language: Portuguese **Borrar** is *smear* and Spanish **Borrar** is *erase*.
Carried straight across, the Spanish shelf would name the smudge brush "erase"
and leave the erase brush with the smudge's name. The two swap places —
`Apagar` → `Borrar`, `Borrar` → `Difuminar` — and a test says so.

**Not yet translated**: the rest of the vocabulary the domain names — combine
operations, view presets, mask operations, gizmo modes, extrude sides, falloff
curves. Those are **62 further label arms across 14 enums**, so an English
interface still shows `Relevo`, `Quadrática` and `Perspectiva` in the option
and viewport bars. See *Not built yet*.

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
- Which shapes were *placed* live in a side-car, `<name>.clay.objects`,
  beside the document rather than inside it: the `.clay` format is the
  engine's, and which nodes a sculptor put there is this application's own
  bookkeeping. Send someone the `.clay` on its own and it opens and sculpts
  with every boolean intact — the hole a cylinder cut is still a hole — but
  none of its shapes is offered as an object any more, so none can be
  selected, moved or re-combined. A row that will not parse costs that one
  object and not the file.
- Session state lives in Application Support on macOS and `$XDG_STATE_HOME` on
  Linux. State, not cache: losing it costs work.
- The **reference images** are session state too: each plane's file path and
  placement, written down beside the recent list. The path and not the pixels —
  a cache of the images would be a second copy of someone else's photograph,
  kept without being asked. A file that has since moved is dropped on the way
  in, the way a recent document that is gone is, and a line that does not parse
  is dropped rather than defaulted: a reference placed somewhere the sculptor
  did not put it is worse than one that is simply gone.

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

**The rest of the domain's vocabulary, in more than one language.** The brush
names go through the string tables now. The other 62 label arms across 14 enums
— `Combine`, `BlendProfile`, `ViewPresetKind`, `RefPlane`, `MaskOp`,
`GizmoMode`, `ExtrudeSide`, `Falloff` and the rest — are still Portuguese
literals returned from `clayspace-model`, so the option bar and the viewport bar
stay Portuguese whatever the menu says. `Strings::tool` is the shape the rest should follow.

**Bezier handles on a curve.** The curve tool offers three of the engine's four
joins. The fourth is a cubic shaped by handles, which needs two more draggable
things per point and a way to break their symmetry — a tool of its own rather
than a setting. Worth knowing when it is built: the engine keeps handles in the
item's *local* space and says so pointedly, because 3DCoat keeps its in screen
space and "its own users call that a wart" — the curve then means something
different depending on where the camera was.

**A curve reopened after it is applied.** Applying lets go of the control
points and leaves an ordinary item. `clay_layer_stroke_points` reads a placed
guide back — "the exact arguments the call above takes, so what comes out goes
straight back in" — so picking a placed tube and editing it again is reachable;
nothing does it yet.

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
mesh layer* — but it cannot *compose* as a mesh: it is not an operand of a
boolean, a blend or a deformer belonging to another layer until it is sampled
onto a lattice, and paying that quantises the exact vertices and drops the edge
loops, which is precisely what made it worth keeping as a mesh.

What has changed is where a sculptor meets that. Choosing an imported model in
the shapes panel places it as a boolean operand and states the crossing's costs
first — the same figures the conversion panel states, because it is the same
crossing. The mesh layer stays where it is and stays sculptable; what is placed
is a sampled copy. So the *surface* still does not compose, and the model can
be used as though it did, at a price that is shown before it is paid.

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

**Pinholes in the brick cache's mesh on a heavily worked surface.** Reported
alongside the triangle loss above, and *not* the same defect — that one is
fixed and this survives it, which is why the artifacts improved without going
away. This one is the engine's.

Reproduced by `visual_holes.rs`, which renders the form and looks at it rather
than comparing two of our own structures. At around six hundred thousand
triangles a mixed session leaves a handful of one-pixel holes, and the pixels
are background: `[34, 37, 42]` against a background of `[35, 38, 43]`,
surrounded by lit surface on every side.

Where they are not is what identifies them:

| what was rendered | holes |
|---|---|
| the incremental store | 8 |
| the same document rebuilt from scratch | 9 |
| **the brick cache's own mesh, uploaded whole** | **9** |
| `clay_document_mesh` of the same document | **0** |

The third row is the one that matters: it skips our splitting, our per-key
store and our slots, and the holes are still there. So they are in
`clay_brick_cache_mesh` rather than in anything this application does with it —
consistent with the header's own distinction, where `clay_document_mesh` is
"the watertight, 2-manifold export path" and the brick cache is the frame path.
Nothing in shading can fill a hole, so there is no mitigation here worth having:
turning normals toward the eye, which would hide a badly shaded triangle, moves
the count by one.

A related but separate cost, worth fixing on our side: the incremental store
holds **55 duplicate triangles** out of 597,521. The engine's header asks a host
holding geometry per brick to dedupe by triangle, because a straddler "may move
to another key's share when a later request names a different set". We now do,
at relayout. It is nine thousandths of a per cent of the buffer — kept because a
relayout already rewrites everything so the pass is free, not because it pays.

**55, and not the 11,333 first reported here.** That figure came from a dedupe
keyed on the three vertex positions, which counts every pair of triangles at the
same three points. Most of those are not one triangle twice. They are two
meshings of one patch of surface with different brick neighbourhoods, and
because vertices are welded across a seam, the normal at a welded vertex depends
on which bricks the call covered — so the two copies sit at identical points and
are shaded differently. Dropping one is not tidying, it is picking a shading,
and it made a settled surface differ from a full re-mesh by twelve levels along
a thin trace of the seams. The key is the whole vertex now: position, normal,
colour and mask, every attribute the shader reads, so a dropped copy provably
cannot change a pixel.

Two lessons, both about instruments rather than about the bug. The check that
pruning lost nothing was written in the same terms pruning matched on, first a
1/4096 tolerance and then positions alone — asked that way it can only answer
no, however wrong the terms are. And the whole thing was invisible on Linux and
CPU: it took the macOS runners to show it, because which copy a store ends up
holding depends on meshing order. A test that passes everywhere it is run is not
the same as a test that passes.

**The 11,278 that are left is the more interesting number.** Coincident
triangles carrying different normals are two per cent of the drawn surface,
shaded two ways, with the depth test picking between them by draw order — and
draw order moves when slots move. That is a candidate for the specks below, and
it is not yet investigated.

**Fixed: the incremental surface used to lose a few triangles a rebuild has.**
Reported as small holes and torn-looking seams while sculpting. Kept here
because the shape of it is worth remembering.

A triangle was filed under whichever key's *vertex* range held its first
corner. The engine welds vertices across brick seams — "a triangle in one key's
index range may reference a vertex in an EARLIER key's vertex range" — so a
triangle could be filed under a key holding **none of its corners**. Nothing
looked wrong that frame. The damage came later, when that key was replaced by a
request whose bricks the triangle did not touch: it was cleared, and nothing
re-emitted it, because the engine only returns triangles with a corner in a
requested brick. Hence a hole appearing minutes after the stroke that caused it.

A triangle is filed under the key whose *index* range the engine listed it in.
That is the engine's own attribution, which by contract holds a corner of the
triangle, so any later request naming that key re-emits it before replacing it.
The ranges partition the mesh, so this files each triangle exactly once — and it
needs no binary search, which the old rule did per triangle.

Two things made this hard to see, and both are worth avoiding again. The losses
depend on the **order keys are requested in**, because the order changes which
key a triangle is filed under and therefore which later request can clear it —
so an early measurement taken over a `HashSet` varied between runs and sent the
investigation after a difference between two engine queries that do not in fact
disagree. And `settle_needed.rs` asks exactly the right question of six gentle
dabs on a fresh sphere, where no triangle is ever filed across a seam that a
later request will clear. `incremental_stress.rs` is the harder version: three
strokes, 52 dabs, and a bisect that finds the smallest losing case.

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

