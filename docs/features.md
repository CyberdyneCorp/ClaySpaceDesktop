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

All twenty-one are bound and each is covered by a before-and-after capture in
`target/visual/`. Which of the three representations each one reaches is in the
Layers column: fourteen have an SDF verb, thirteen a voxel one, and seventeen a
mesh one.

| Tool | Engine verb | Layers | What it does |
|---|---|---|---|
| Padrão | `clay_layer_apply_stroke` with relief | all three | Displaces the surface along its normal |
| Inflar | `clay_voxel_sculpt_inflate` / relief, wider and softer | all three | Swells the footprint; a negative amount erodes. On a field it is relief like Padrão — the engine binds both to it — with a region and rim 1.35× the brush and 0.32 of the lift, so it swells where Padrão ridges |
| Suavizar | `clay_sdf_smooth_*` / `clay_item_volume_relax` / `clay_voxel_sculpt_smooth` | all three | Relaxes the surface. Live on the field side, through a transaction |
| Mover | `clay_sdf_move_*` / `clay_layer_move_surface` | SDF, mesh | Drags the assembled surface. Buds rather than stretches. Live on the field side, through a transaction |
| Mover Topológico | `clay_item_volume_move_topological` | SDF | The same drag with its reach measured **along the material** rather than through space, so a part close in space and far along the surface is left behind. It bakes, so it costs more than Mover and is the one to reach for when the cheap drag pulls something it should not |
| Pinçar | `clay_voxel_sculpt_pinch` | voxel, mesh | Moves surface cells toward the brush centre |
| Raspar | `clay_voxel_sculpt_scrape` | voxel, mesh | Flattens and smooths from one snapshot |
| Planar | `clay_item_volume_flatten_from`, cut-only / `clay_voxel_sculpt_flatten` | all three | Planes without filling on a field and a mesh, which keeps a facet crisp. **On a grid it is two-sided** — material above the plane goes and hollows below it fill — because that is the verb the grid has; the tooltip says so rather than faking cut-only |
| Preencher | `clay_voxel_sculpt_fill_cavities` | voxel | Fills narrow pockets |
| Camada | `clay_layer_apply_stroke`, clamped | all three | A stroke that does not build up on itself |
| Máscara | `clay_mask_apply_stroke` | all three | Freezes a region against every verb. Invert, clear, expand, contract, smooth, bounded complement and extrude are in the Máscaras menu |
| Puxar | swept-sphere chain on a Catmull-Rom curve | SDF, mesh | Pulls a tendril out, tapering to its tip |
| Polir | `clay_item_volume_flatten_from`, cut-only | SDF, mesh | hPolish |
| Relaxar | `clay_item_volume_relax` | SDF, mesh | Relax as a brush |
| Nudge | `clay_voxel_sculpt_smudge` | voxel, mesh | Drags the surface skin, leaving the interior |
| Trim | `clay_cut_create` | SDF | A shape drawn on the frame, cutting through |
| Argila | `clay_layer_apply_stroke` with relief and buildup / `clay_mesh_sculptor_stamp` (CLAY) | SDF, mesh | Builds up in flat-ish planes, the way clay is added by hand. On a field it is relief with **buildup** accumulation and a denser stroke, which is what separates ClayBuildup from Standard in ZBrush too — a second pass adds where Camada's does not |
| Vinco | `clay_layer_apply_stroke` with incise / `clay_mesh_sculptor_stamp` (CREASE) | SDF, mesh | Pinches a sharp ridge or trough along the stroke. On a field it is `Op::Incise` — "a thin region gives the line", in the engine's words — at 0.6 of the brush, which cuts to the full depth in three fifths of the width. Held, the key raises the ridge it would have cut, which is the inverse the engine names |
| Pintar | `clay_voxel_paint_brush` / `clay_mesh_sculptor_stamp` (PAINT) | voxel, mesh | Writes colour rather than moving the surface. The colour comes from the swatch in the options bar, which is shown for the two tools that read one |
| Borrar | `clay_mesh_sculptor_stamp` (SMEAR) | mesh | Drags the surface sideways without carrying it away |
| Apagar | `clay_voxel_erase_brush` | voxel | Removes cells |

**Padrão and Inflar are two marks on a field.** ClayCore's own equivalence
table binds both to `Op::Relief` — relief moves the accumulated surface along
its own normal, which is what either does — and the application passed the same
stamp for either, so two brushes on the shelf drew one thing. What tells them
apart in ZBrush is the profile: Standard raises a ridge that follows the
falloff, Inflate swells the whole footprint, broader and lower at the rim. So
Inflar's region and rim are 1.35× the brush and it asks for 0.32 of the lift;
Padrão keeps the engine's standard clay mapping, k = rounding = radius.

The 0.32 is measured, not chosen. Raycasting a grid at the mark on the starting
form with a 0.25 brush, as peak height above the sphere and footprint area:

| binding | peak | footprint | height ÷ width |
|---|---|---|---|
| Padrão, k = rounding = r | +0.180 | 1179 | 0.0053 |
| Inflar at 0.8 of the lift | +0.238 | 1939 | 0.0054 |
| Inflar at 0.32 of the lift | +0.173 | 1772 | 0.0041 |

The middle row is the trap: a wider region under buildup accumulation lifts each
point through more stamps, so the first attempt came out wider **and taller** —
the same ridge drawn with a bigger brush, which is not what Inflate means.
`visual_sdf_symmetry` asserts the *shape* — half again the footprint at a fifth
less slope — rather than counting pixels, which a merely bigger mark would pass.

They stay closer here than in ZBrush, and that is the engine's design rather
than a setting: relief is the only op that moves an existing surface along its
own normal, so both brushes are relief and only the profile can differ. A true
per-stroke inflate — offsetting the field inside the region — would need a
verb ClayCore does not expose.

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

### Brush colour

One current colour, plus the last six before it, shown as a swatch in the
options bar. It appears for the two tools that read one — Pintar and Borrar —
and is hidden for the eighteen that do not, because a control that does nothing
is worse than an absent one.

**Shared across tools, unlike every other brush setting.** Size belongs to the
tool and to the representation: a size that suits a grid's cells is not the one
that suits a field, and a small detail brush should stay small when the
blockout brush is made large. A colour is the opposite — it is what you are
painting with right now, and every colour brush picks up the same one. Held in
`BrushSettings` it would have been four values for one question: Pintar and
Borrar disagreeing on a mesh and again on a grid.

**A grid stores palette indices**, so the adapter resolves the colour to an
entry before painting: an existing entry within half a step of an eight-bit
channel is reused, and only a genuinely new colour adds one. Without that
tolerance a colour wheel adds an entry per stroke — it returns values a float
apart as the pointer moves inside one pixel — and the engine caps a palette at
255, past which the nearest entry is used rather than the stroke being refused.

**A structural deposit keeps the neutral clay tone.** "Put material here" and
"put *this colour* here" are different instructions, and a sculptor blocking out
with a red swatch chosen should not find every dab red.

The swatch edits in sRGB and the engine stores linear, so the two are converted
at the one place they meet. Painting moves no vertex, honours the mask and the
symmetry, undoes as one gesture and survives the document.

### Symmetry

Symmetry about X, Y and Z reaches all three representations, and by two
different mechanisms because the representations are two different things.

**Symmetry belongs to the subtool, not to the document.** The toggles read and
write the *active* subtool's axes, and switching subtools restores that
subtool's own setting rather than carrying the previous one's along. A new
subtool starts with X on, which is what the design asks. One exception, and it
is the rig's: a rig's own subtool starts with symmetry **off**, because a rig
does its own mirroring (`add_zsphere` places the reflected node itself) and a
layer mirror on top of that hangs a second arm off the first.

The setting and the engine's mirror are two things, and the mirror is written
by the stroke that wants it rather than by the toggle that asked for it.
Pointing a layer mirror is an *edit* — measured, `clay_set_layer_mirror` takes
the undo depth from 0 to 1 — so it belongs inside the gesture, where the
ViewModel counts it and one undo spends it along with the rest. Written beside
the gesture, it would sit on the engine's stack unaccounted, and the next undo
would spend itself on the mirror and leave part of the stroke standing.

A document reopened from disk records symmetry as **off** on every subtool, and
so does a fresh subtool's record of what the engine has been told. The file
does keep the mirror — measured, an item mirrored before a save is still
mirrored after a reopen — but the ABI has no call that reads one back, so what
is recorded here is an assumption either way. Off is the assumption that
changes nothing: the first stroke that wants otherwise writes through, and
writing the default over what was loaded would be worse, since a mirror applies
to items added before the call as well as after and a form saved unmirrored
would come back mirrored.

On a **field**, through the layer's mirror — `clay_set_layer_mirror` reflects
the layer's items, so both halves belong to one operation and undo together.
That covers the brushes that *add* an item: Padrão, Inflar, Camada and Puxar.

The five that **rewrite the field** rather than adding an item — Mover,
Suavizar, Relaxar, Planar and Polir — cannot be reached by the layer's mirror.
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

A mask belongs to **one subtool**, and to no representation. It is a
world-addressed field the verbs consult, so it is painted the same way and
honoured the same way on a field, a grid and a mesh — and it is the *active*
subtool's mask that is presented and applied. Switching subtools neither
discards the mask you painted nor applies it to the form you moved to; coming
back finds it covering what it covered, and neither mask gates edits on the
other subtool. `subtool_state.rs` holds that.

Neither half was true before. Being of one subtool is new here; being of no
representation was claimed and not kept — on a grid the tool fell through to the
depositing arm and *added clay* where the sculptor asked to freeze a region, and
on a mesh it was refused outright even though a mesh stroke had been handing the
mask to the engine all along. `masking.rs` holds both of those.

**A mask survives closing the document.** It belongs to the layer inside the
engine's own document — `clay_document_add_mask` attaches it and
`clay_document_save` writes it — so painting one, saving, closing and opening
again finds the same region frozen and still gating.

**And it now protects against the *operation*, not only against the brush.**
Those are two different things, and until the ClayCore 0.73.0 pin only the
first worked. A mask gates *authoring*: a stroke consumes it as it becomes
items, so a brush does not deposit where you painted. It said nothing about
what those items then do — so a **subtracting** stroke crossing a masked ear
took the ear anyway, which is precisely the case a sculptor paints a mask for.
Measured through the application now: an unmasked subtracting stroke takes the
centre of the starting form from 1.0 to **0.825**, and a masked one leaves it
at **1.0**.

The entry point that does this, `clay_item_set_gate`, had been in this
codebase's engine wrapper doing nothing since v0.39.0 — accepted, and inert at
every width and threshold tried. The cause was never the tuning: the gate was
placed by the transform of *the item it protects*, while the mask it measures
is stored in world units, so a cut with a placement carried its protection away
from where the mask was painted. Fixed upstream as
[#394](https://github.com/CyberdyneCorp/ClayCore/issues/394), and the header
now states the rule the fix rests on — the gate is in world space and does not
travel with the item, so it can be set once on a stroke's template and be right
for every mark the stroke makes.

The protection **fades** rather than stopping at a step, across four cells of
the brick cache. That is not a softness setting: the engine measures the mask
into a distance and derives the falloff from that width, because a step in the
field has no finite bound and nothing could march it.
`mask_persistence.rs` is the round trip, and `claycore_mask_persistence.rs` is
the boundary measurement underneath it.

It did not, and the reason was never the engine. `Document::mask` handed back a
mask borrowing the document — it had to, since the handle may not outlive it —
while every masked verb in the wrapper wanted that handle *and* the document
together: `apply_stroke`, `relax_region`, `flatten_region`, `mask_extrude`, and
a voxel grid borrowed out of the same document. The C side is built for exactly
that pairing and Rust cannot spell it, so each subtool's mask was a standalone
`clay_mask_create` beside the document and went away with the window.

What fixed it was addressing the mask by the **identity of the layer it belongs
to** rather than lending the handle:

- `MaskSource` names nothing, a caller's own field, or a layer of the document
  being edited; `apply_stroke` and `mask_extrude` take one and resolve it
  inside the wrapper, where the two pointers coexist for one C call.
- `Document::layer_mask` lends a `MaskLease` through a **shared** borrow, which
  the relax, flatten and mesh paths hold beside another read of the same
  document.
- `Document::voxel_layer_masked` hands over a grid and its layer's mask out of
  one borrow, since a grid takes the document exclusively.

Two things follow that are worth knowing. A document has no verb for
*detaching* a mask, so **Limpar empties it and it stays attached** — the panel
keys on whether anything is frozen rather than on whether a mask exists, and
"there is no mask" is now only reachable on a subtool nobody has painted one
on. And a mask edit records on the engine's history, so **one mask gesture is
one undo**; before, an undo after a mask stroke spent itself on whatever came
before it.

### Freezing a region by drawing round it

The brush is one gesture and it is not always the right one. *This limb, not
that one* and *everything above this line* are what a mask is usually wanted
for, and scrubbing a brush over them ends in a ragged boundary a minute later —
on the near side only, because the far side is behind the form.

So the mask brush has two more gestures. **Gesto** on the options bar, and the
same three in the Máscaras menu, chooses between **Pincel**, the drag it has
always had, **Laço**, a shape traced freehand over the form, and **Retângulo**,
a box dragged corner to corner. Draw one and everything it encloses freezes. Not
three tools — one setting on one brush, which is where ZBrush keeps them.

The rectangle is not a lesser lasso. A hand cannot draw a straight line, and
"everything above this line" is the most common thing a mask is wanted for. It
is square to the **screen** rather than to the world — it is drawn on the
screen, and a box that came out lozenge-shaped because the camera was turned
would be a box nobody could aim — and it is the box between the corner pressed
and the pointer now, whichever way round they are and however far the hand
wandered on the way.

Past the pointer the two are the same thing. A lasso keeps every point it passed
through and a rectangle keeps two corners, and that is the only place they
differ: neither the containment test, nor the traversal, nor the engine can tell
them apart.

It freezes **through the form**. The outline is drawn on the screen, so the
surface behind it freezes with the surface in front of it, in one gesture,
without turning the model. Hold **Ctrl** and the same shape *releases* what it
encloses instead, which is how a mask is trimmed back rather than cleared and
repainted. Which of the two it will be is decided when the drag begins and held
for the whole of it, as a stroke's modifiers are — and so is which gesture is
drawing, so switching to Retângulo mid-drag abandons what was traced rather than
reading it as two corners.

The shape is drawn in the accent while it will freeze and in a grey while it
will release. A lasso's closing edge is shown faint, because a traced outline
closes across a gap the sculptor can see and showing where it lands is what
makes it predictable; a rectangle's four edges are all solid, because it has no
such gap and drawing one of them faint would suggest it were less certain than
the other three. A press beside the form begins a shape rather than turning the
camera — an outline is drawn *around* a region — and a click that never became a
drag does nothing quietly.

**The brush ring goes off while a shape is being drawn**, by the same rule that
takes it off under a raised cage: a ring says the next press leaves a stroke
where it sits, and with a lasso or a rectangle in hand the next press draws a
line on the screen — which is not a thing the surface has a footprint for. The
outline being traced is the only feedback the gesture needs.
`input::shows_the_brush_ring` is the one place that decides it, asked by the
press and by the ring alike so the two cannot come to different answers.

Not while a cage is up, though. A cage owns the viewport and the brushes are off
under it — [A cage is a mode](#a-cage-is-a-mode) — and a press that takes hold of
no control point draws the cage's own selection box. A mask gesture that took
that press would be the one brush still reaching past a raised cage.

The region is the outline swept **straight** along the view direction rather
than out from the eye. That is the engine's own rule for the cut tool: *"a trim
is a straight cut, as it is in ZBrush and 3DCoat"*, because a region defined by
a converging wedge depends on where the camera was standing. It is bounded by
the active subtool's own extent, and it belongs to that subtool like every other
mask.

**A whole lasso is one undo**, and that is the reason it is built the way it is.
A document-owned mask snapshots its whole chunk map for the history on *every*
call that writes to it — about four milliseconds a call on a mask covering a
million cells, against seven microseconds on a standalone mask that records
nothing. A region delivered as five thousand small writes takes twenty-one
seconds. So it is delivered as **one stroke**: a path that visits every column
of the region, walked by the engine's own stamper. One call, one snapshot, one
entry in the history.

The path is a depth-first walk, and it never leaves the region. That is not
tidiness: a stamp lands everywhere the path goes, so a connector cutting across
a concave outline would freeze a stripe nobody drew — which is exactly what a
plain back-and-forth over the rows does the first time a lasso is thrown round a
C. `mask_outline.rs` draws one and checks that the opening is still free.

**What a lasso costs is the volume it sweeps.** The lattice the path is walked
on is aligned to the camera, because that is where the outline was drawn, and a
brush footprint is aligned to the world; a ball has to reach half the pitch's
*diagonal* to cover the lattice from any angle rather than half its side, so
every cell of the region is written about 2.7 times, at about 140 nanoseconds a
write. Sized to half a side instead, the two tile only when the camera happens
to face down an axis, and from anywhere else the frozen patch comes out speckled
with cells no stamp reached. A ball rather than a cube is worth 40% of the
gesture — a cube of the same reach spends 5.8 writes per region cell against
2.7, all of the difference in corners that overshoot it, and the pair measures
1191 ms against 800 on one machine. `mask.outline` measures the extreme on a
quiet one: an outline thrown around the whole of the reference form, 659 ms. An
ordinary lasso over part of a subtool is a fraction of that.

**The edge is quantised to two mask cells**, 0.04 world units at the mask's own
0.02 pitch, and the pitch is not a dial: opening it by two divides the stamps by
eight and multiplies the cells each one writes by eight, so it buys the edge and
nothing else. A region too large to write at all is therefore *refused* with a
reason rather than quietly coarsened — a lasso around a subtool tens of units
across runs to hundreds of millions of cells, and the honest answer is to say so
and ask for a smaller outline. `visual_mask_outline.rs` is the picture, and it
also holds the two gestures to each other: a dragged box has to land where a
traced outline round the same region lands.

Symmetry does not reach it, exactly as it does not reach the brush: a mask is a
world-addressed field, and neither gesture is mirrored.

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

**Pintar colours cells that are already there**, which is what makes it the one
verb on a grid that adds nothing: the palette always exists, so painting a cell
creates no storage — unlike on a mesh, where the colour attribute is twelve
bytes a vertex and is refused rather than created. Painting a cell the colour
it already carries still reports `changed: false`, which is honest rather than
a broken binding, and `brush_colour.rs` is where the colour question is asked
properly.

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
readout measures both, and a save writes both. When the topology stops taking
what you are asking of it, *When a mesh has stopped taking detail* is the way
back.

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

**Smoothing is no longer held.** ClayCore 0.60.0's transaction samples the
layer once when the pointer goes down and relaxes its own retained volume per
dab, touching nothing in the document. So Suavizar and Relaxar show themselves
while they are being made, and a stroke is still one action to undo. Measured
on the starting form at the application's own 0.02 sampling: **186 ms** to open
the gesture, **~5 ms** a dab.

The transaction's own commit is *not* used. It installs the working volume as
the layer's one item, consolidating the whole subtool on every stroke — heavy
everywhere, since it discards the edit list and re-samples at the cache's cell
size, and measurably damaging on Metal (roughness 7.82 against a ceiling of
6.00, where the same stroke leaves 5.74 here; ClayCore#379). The stroke is laid
down by the bake that was always used, which reproduces the old numbers exactly
on every backend. The preview and the result are therefore different
computations of the same smoothing, and they land 0.09 apart in roughness —
close enough that the surface does not visibly move when the pointer comes up,
and `live_smooth.rs` holds it to that.

What the viewport draws in the meantime is the engine's own mesh of the
transaction's own samples. The preview's lattice has an origin of its own — the
layer's bounds, less the padding — which does not land on the brick cache's
lattice and cannot be made to, since one padding cannot align three axes whose
bounds have different remainders. So the preview is **relabelled rather than
resampled**: it keeps a cache of its own, preview brick *K* is stored as that
cache's brick *K*, and the constant translation between the two lattices is
undone on the vertices. Nothing is interpolated, and
`live_smooth::what_the_preview_showed_is_what_the_commit_installs` is what
holds that to being exact rather than close.

Two conditions, and the second is about what the preview is *of*. The layer has
to be an editable field, and it has to be the **only visible field subtool**:
the brick cache holds the hard union of every visible SDF layer and attributes
no brick to the layer it came from, while a transaction previews one layer
alone. With a second one in the document the gesture falls back to being held
whole — correct, just not live.

**Move is live too, and it is where the transaction pays most.** Mover does not
bake; it warps. Each drag appends a `grab` to the deformer chain of every item
it reaches, and the engine's Lipschitz bound for a chain is the **product** of
its links — so a chain that grows costs the marcher geometrically, not
linearly. Writing one grab per *segment*, as the application did through
0.60.0, therefore made a drag cost the field as much as it was finely cut.
Measured on the starting form, twelve drags of six segments each:

| delivery | deformer chain | safe step scale |
|---|---|---|
| one grab per segment | 72 | 0.000608 |
| one grab per gesture | 12 | 0.002456 |

The second is what `clay_sdf_move_*` gives: the edit list is walked **once**
when the pointer goes down, every frame after that costs only the items the
drag moves, and the commit rebuilds one chain per item from what was captured
at the anchor — so the gesture costs the same however many segments drew it,
which is what `live_stroke::a_drag_shows_itself_and_costs_the_field_the_same_
however_it_is_cut` holds. It also fixes a correctness bug on the way: an update
takes the drag **measured from the anchor**, never an increment, where segments
each anchored where the last one stopped and composed into a pull further than
the gesture ever asked for.

Unlike Smooth, this one does not care how many field subtools are visible,
because its preview is not drawn from a lattice of its own.

**Drawing a drag the document does not carry.** A Smooth transaction hands over
sampled bricks; a Move transaction hands over no samples at all. ClayCore's C++
class exposes a `preview_layer()` for exactly this and **the C ABI does not
carry it**, so the application takes the other route the header invites — the
resolved grabs, "so a host can reproduce the preview through machinery it
already has". Once per segment: write the grabs the current total resolves to
onto the layer, let the brick cache sample the dragged surface out of the
document, and undo them again. What stays on screen is the cache, which keeps
what it was last given until something marks those bricks dirty.

Undoing inside the same segment is not tidiness. It is what makes the commit
legal — a commit re-checks a stamp derived from the layer's **content** and
refuses a layer that moved underneath it — and it is what keeps the history
honest, since the ViewModel counts a live segment by the undo depth it left
behind. A segment that kept its preview would be counted as having written it,
and cancelling the drag would then spend one undo per segment against history
the gesture never made.

The mirror is the engine's here and not the application's. `baked_stroke`
reflects a gesture and runs the verb once per image, because the layer mirror
cannot reach the verbs that rewrite a field; `clay_sdf_move_*` states that it
reflects the drag into every image the layer emits and resolves one grab per
image, so the live path does **not** reflect it again. Measured, both routes
pull each side by the same 0.1345 — `move_mirror.rs` holds both.

It costs about 17 ms a move on a 140,774-vertex mesh, nearly all of it the
stamp itself rather than the buffer it fills (1.2 ms). Dropping the surface
walk would take it to 12.8, and is not taken: with the single-stamp path the
walk is what makes Move topological, and `mesh_move.rs` fails without it. The model takes back what the last
segment did before laying the gesture down again, which is what keeps one drag
to one undo: only the release banks anything, and a cancelled gesture takes its
preview with it.

**Only for the verbs that are delivered that way.** A stamping verb is sent
just the samples the model has not seen, so it has nothing to take back, and
taking the last segment back anyway erased the stroke as fast as it was drawn —
a drag kept only its final dab and the brush read as one that has to be clicked.
Its record is *continued* instead, which `MeshDeltas` is built for: it coalesces,
so a stroke passing over the same vertex forty times still records where it
started once, and the gesture is still one undo that puts every vertex back
exactly.

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

**Taper and twist are chips with their shape on them** — a section that
narrows, a band that turns — in the interface's language rather than the
domain's Portuguese, which is what the panel read in every locale before.
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

While one is up the layer is being *deformed*, and four things follow from
that:

- **It does not follow you to another subtool.** A cage is sized to what one
  form contains, and that box means nothing around another, so changing the
  active subtool while one stands **resolves** it: an untouched cage is exactly
  the identity and is taken down without asking, and a dragged one puts the
  question — deform and switch, discard and switch, or stay here. The model
  drops a cage that reaches the switch unresolved rather than re-drawing it
  around a form it was never fitted to.

- **The brushes are off, and so is the ring that promises one.** A press that
  misses a control point does not sculpt. It used to fall through to the brush,
  so a slip while aiming sculpted the very form the cage was there to bend —
  and the strokes it left made the next control point harder to hit. The
  *cursor* went on being drawn over the form for longer than that: the routing
  refused the stroke and the orange ring went on offering one, which is the
  worst of both, since a sculptor aiming at a corner handle could not tell
  whether a slip would leave a mark. A ring says "the next press leaves a
  stroke here", so it is drawn only where that is true — the same rule the
  whole-subtool manipulator already followed, now written once as
  `input::shows_the_brush_ring` rather than twice.
- **A press that takes hold of nothing draws a box.** Not every miss is a
  mistake: a cage is worked a face at a time, and gathering a face by
  Shift-clicking four or eight corners is four or eight chances to miss. So the
  primary button drags a rubber band across the viewport and takes every
  control point inside it — including the ones behind the form, which is the
  whole reason the cage is drawn through. Held, **Shift** adds the box's catch
  to the selection instead of replacing it, so a band and a click mix freely. A
  press and release in one place is not a box but a click on nothing, which
  clears the selection and puts the manipulator away; three points of travel
  tell the two apart, so a hand's tremor is still a click.

  The camera keeps working: **the secondary button and the orbit modifier both
  orbit**, so the cage can still be turned to look at from behind without being
  taken down. That is what the old rule — a miss orbits — was for, and it is
  the trackpad's route as much as the mouse's.
- **The form is drawn through.** Half the control points are behind it, and a
  solid surface hides exactly the handles that need reaching. Blender's X-ray
  and ZBrush's Ghost do the same thing for the same reason. Seen through, not
  turned off: the form stays readable as a form. A ghosted surface writes no
  depth, which is also why the cage keeps full-strength handles while the
  manipulator on solid clay is drawn faint where the clay is in front of it.
- **Handles keep their size.** They are sized from the box the cage was *built*
  with, not from where its points are now. Sized from the current extent — as
  they were at first — hauling one corner out inflated every other handle, so
  the targets a sculptor was aiming at swelled under the pointer as they
  worked.

### The manipulator

A click selects one control point; **Shift-click** adds or removes one without
disturbing the rest; and a **drag across empty space** takes every point the
box encloses. That is what the manipulator exists for — dragging points one at
a time needs no widget, and turning a whole face of the cage cannot be done
without one. A box is resolved when the pointer comes up rather than as it is
drawn: a selection that changed under a moving band would drag the widget to
the middle of whatever was momentarily inside it, and it would wander across
the screen while the box was still being drawn.

It sits on the **middle of the selection**, not on the last point picked, so
adding a point moves the widget to where the selection is.

**One widget carries every operation** — ZBrush's Gizmo 3D. Along each axis an
arrow that slides, a ring that turns and, on a cage, a box that scales; the
outer ring that turns in the screen plane; the centre block; and four corner
brackets framing the widget's extent. The operation is chosen by the handle
grabbed, not by a mode set first: three modes drew three different widgets
once, and the chips became a step a sculptor had to take before every move.
The **Mover / Girar / Escalar** chips still exist, for the two gestures no
handle names — what the centre block does (a slide in the view plane, or a
uniform scale) and what a press on the clay does while a whole subtool's
widget is up — and they follow the last handle grabbed, so they say what the
last gesture did and what the next press on the clay will do.

Shapes rather than colours alone carry the meaning: an arrow slides, a ring
turns, a box scales. A person reaching for a handle is not reading a legend,
and the three axis colours are the one part of this a colour-blind sculptor
cannot use. The rings sit inside the arrows' reach and the boxes inside the
rings, so the three are told apart by radius as well as by shape; the outer
ring stays outside everything at 1.28 of the reach, and the picture and the
hit test read the same constants.

**It is seen through the form it stands on.** Every part of the widget is
drawn wherever it is — a handle behind the clay is never hidden, which is what
makes the far half of a cage reachable at all — but a part with the sculpt in
front of it is drawn **faint**. Drawn at one strength everywhere, a rotate ring
around a head reads as a circle painted on the frame; faint on its far half, it
reads as a hoop the head passes through, and which way it will turn under a drag
becomes something a sculptor can see rather than infer.

Faint says *behind*, never *unavailable*. The hit test walks every handle by ray
and ignores depth entirely, so a handle drawn faint is grabbed on exactly the
same terms as a bright one; that is also why it is not drawn fainter still, since
past a point a pale handle reads as disabled, which would be a lie about what a
click does.

It is a comparison against the depth the clay wrote, and the clay is the only
thing that writes depth. So a widget over empty space, over the grid, over a
symmetry plane, over a reference photograph — or over a **ghosted** surface — is
drawn exactly as it always was. That last one matters more than it sounds:
whenever a cage is up the surface is ghosted, so a cage keeps the full-strength
scaffolding it has always had, and the depth cue appears on the case that asked
for it, a manipulator on solid clay. The orientation gizmo in the corner is left
out entirely — it has a camera of its own, and the clay's depth in those pixels
says nothing about where it stands.

**The widget stands over the whole form.** Its arms reach past the target's
own box — an object's outline, a subtool's bounds — with a floor at the
screen-constant size a small target needs and a ceiling that keeps it on screen
when the camera is close, so it encloses what it moves rather than sitting as a
mark in the form's middle.

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
- **An arrow can be grabbed anywhere along its shaft.** Reported from using it:
  the manipulator "only works if you perfectly land the mouse on the axis
  arrow". The arrow is drawn from the pivot to its cone and every part of that
  reads as a handle, but only a sphere at the *tip* was tested — so a press on
  most of what a person could plainly see missed. Worse than missed: a ring
  encircles the pivot, so a ray aimed down the inner shaft passes near the
  ring's **far** side, and the press that was meant to slide the selection
  turned it instead. The shaft is hit-tested as a capsule now, in the same
  nearest-along-the-ray competition as everything else, so the near shaft beats
  the far ring. It is considered **last**, which settles the other half: where
  a handle genuinely sits *on* the shaft — the centre block at its foot, the
  scale box partway out, the two rings that cross it at their own radius — the
  two are the same distance from the eye, and going last leaves the press with
  the smaller, more particular target.
- **The handle under the pointer is lit.** Half a dozen targets overlap on one
  widget, and which of them a press will take was only discoverable by pressing
  — the renderer has carried a `hovered` field all along and nothing but a drag
  ever filled it, so a sculptor aiming at an arrow found out what they had
  grabbed after the fact. It is asked the same question a press asks, every
  frame the pointer moves, so what lights up and what a press takes cannot
  describe different widgets. During a drag the handle *in hand* stays lit,
  wherever the pointer has since travelled.
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
selection box, then the surface**. The manipulator is drawn over the cage and
sits on the selection, and the cage sits outside the form; without that order a
press on the green arrow finds a control point behind it, and a press on a
corner handle finds the clay. The box comes after both and before the clay,
because it is what a press *that took hold of nothing* means — and while a cage
is up nothing behind it, neither an object nor a stroke, is what the press was
for.

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

## Inserting a form, and the booleans on it

The rail's **Formas** button, or *Arquivo → Formas*, opens a section of the
right region offering fourteen shapes — box, sphere, cylinder, cone,
torus, capsule, ellipsoid, pyramid, rounded box, frame, rounded cylinder, hex
and tri prisms, octahedron — each with the numbers it is actually measured by,
which are different numbers for different shapes. **Inserir** puts one where
the pointer is on the surface, or where the camera is looking when the pointer
is off it. The two the engine calls unbounded — a plane and an infinite
cylinder — are not offered: neither has an extent for a manipulator to sit on
or a bound for the cache to work from.

**The shapes are a section of the right region, not a window.** They were a
window floating over the viewport, and the viewport is where the form a shape
is being placed into stands, so the panel hid the very thing the shape was
being aimed at. Docked under the material, the picker and the sculpt are side
by side while a shape is placed and turned; the section is put away from the
`×` on its own heading, as the window was from its title bar, or from the rail
button or menu entry that opened it, each of which pushes the same command. Its
combo boxes take the panel's width rather than the fixed width the window gave
them, and the selected object's three combine chips wrap where Interseção does
not fit the panel's row.

**The three booleans are chips, with the two discs on them.** Unir, Subtrair
and Interseção are what a placed shape is for, so they stand as a row above the
full list of operations — the outline of both discs, the crescent one leaves,
the lens where they overlap — and a sculptor does not open a list of thirteen
to find "cut". The list keeps the rest: grooves, pipes, shells and the others.

### As a subtool, or into the one being worked

*Inserir como* is two chips and the panel's first control, because the choice
comes before the shape. **Novo subtool** — the default — makes the form a
layer of its own: active on arrival, so the next dab lands on it, and standing
where the pointer was, with the layer's own middle on the form so the
whole-subtool manipulator sits on what it addresses. **No subtool ativo** puts
it into the active layer as an item, which is how the parts of one form are
built.

Both are wanted and guessing between them from context would be wrong half the
time, so neither is inferred. An object needs an SDF layer's ordered list and a
grid or a mesh has none — the panel says so beside the chips rather than
refusing after the click, and the *subtool* destination stays available there,
since nothing stops a new field layer standing beside a grid.

The layer and the form in it are **one undo step**. They are two engine edits,
and without the group one ⌘Z would take the form away and leave an empty
subtool standing. Names are derived rather than asked for, and made unique:
a voxel layer's grid is reachable only by name (ClayCore
[#365](https://github.com/CyberdyneCorp/ClayCore/issues/365)), so two subtools
sharing one shadow each other's grid and a stroke lands on the wrong one. Every
route that creates a layer derives its name that way — the crossing included,
which is the route that actually makes most voxel layers.

### Two more sources

**Importar malha como subtool** reads a file and stands it in the scene
carrying its triangles — a mesh layer, sculptable with the fixed-topology
brushes, movable with the manipulator, and active when it arrives.

Movable is worth spelling out, because a carried mesh is the one representation
the engine's own layer transform does not reach: a mesh layer contributes
nothing to the field, so the tape has no item to move. The application applies
the transform itself, in one place, to everything that crosses between the
viewport and the mesh's own vertices — what is drawn, what a ray picks, the box
the manipulator sizes itself to, the cage, and the mesh when it is an operand of
a boolean. A mesh subtool therefore moves, turns and scales as a whole like any
other, and sculpting it lands where it is drawn.

**Copiar subtool** takes a subtool already in the scene and makes another. The
word is *copiar* and not *instanciar* deliberately: the engine has no
instancing (ClayCore
[#364](https://github.com/CyberdyneCorp/ClayCore/issues/364)), so what this
does is bake the source alone into a volume of its own — the other layers
hidden around the sampling, exactly as the subtool boolean bakes its operands,
and the visibility the sculptor set restored on every exit path including the
one where the bake refuses. The consequence is the point: **sculpting the copy
cannot reach the original.** A subtool with nothing in it is not offered, since
copying it would produce an empty subtool with a name.

The stack's add control asks the same kind of question: **+ Nova camada** makes
the field layer it always made, and the list beside it makes a voxel layer
directly, rather than by crossing one afterwards.

Two entries and not three. A mesh layer is made by *carrying* a mesh and there
is no call anywhere that makes an empty one, so the specification's offer of
"SDF, voxel and mesh" is qualified — *where a mesh source is at hand* — and
when a layer is created out of nothing there is none. That route is the import
above, which makes its own layer. Asked for a mesh layer anyway, the document
refuses by name and says where one comes from.

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

**And the click makes that form's layer the one being sculpted.** A scene holds
several forms — a layer each — and clicking one is how you say which of them
you are working on: the next dab lands there, and the shelf offers that layer's
representation. The same click that selects an object activates the layer it
stands in, and a click on a form carrying no object activates the layer anyway
while the press falls through to the brush. A **ghosted** layer is transparent
to this, because the engine excludes ghosts from the attributed raycast: the
layer *behind* the ghost is the one that becomes active. A **locked** layer is
still pickable, so it activates and then refuses the dab with its reason —
locked is not hidden and not ghosted, and the three say different things.

There is one active layer and not two. Clicking a row in the layer stack and
clicking geometry in the viewport reach the same command, so the selected
subtool and the sculpted one cannot come to disagree; the scene tree and the
stack both light the same row.

A selected object is **outlined** in the viewport. A subtracting object is
behind the surface — what you see of a bore is the hole — and without the box
there is nothing to aim but a manipulator over a cavity.

The starting form is a placed sphere and is listed as one. It always was;
nothing but the absence of a selection model made it special. It can be
selected, resized and deleted like anything else.

### Which subtool is active, read off the viewport

The stack lights the active row, and the viewport says the same thing, because
a sculptor working on a form is looking at the form. Two mechanisms behind one
look, and they are two because the engine offers no third.

A subtool that **carries geometry** — a voxel grid, an imported or converted
mesh — is drawn one layer at a time now rather than as one concatenated buffer,
so the active one takes a warmer material and the rest stay plain clay. The tint
is the accent's hue at full value mixed toward white: the accent as stored takes
two thirds of the value out of the clay, which reads as a shadow rather than as
a cue. It is a *material*, not a vertex colour, so an export carries no trace of
it — `visual_active_subtool.rs` compares whole `v` lines to hold that, since OBJ
writes vertex colour as three more numbers on the same line.

A subtool made of **field** cannot be tinted. Every visible SDF layer meets in
one merged surface, and splitting that per layer needs an attribution the engine
does not offer — so an active field subtool is cued by its **bounds outline**,
drawn like a selected object's box and dimmed against it: which subtool is
active is standing state, and the box around a form just placed is the more
urgent of the two.

**Nothing is cued while one layer is visible on its own.** The point of the cue
is to tell the active subtool from the *others*, and with none to be told from,
a tint says only that the clay changed colour.

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

### The manipulator on a whole subtool

At the head of the options bar, **Transformar** puts the same widget on the
whole active layer: it moves, turns and uniformly scales everything the layer
holds as one engine transform, and the drag is one undo step. Pressing the lit
chip puts the manipulator away. It stood under the layer stack as three chips
first and is one chip at the top of the window now, where a mode a sculptor has
entered can be seen without looking for it.

**W, E and R choose the mode** — Maya's keys and Unity's, and what a hand
coming from either reaches for without being told. A key pressed with no widget
up puts the whole subtool's manipulator up in that mode, as entering the move
tool does there; where a cage, a curve or a placed object already owns the
widget, the key changes *that* widget's mode and takes nothing away from it.
The chip wears the mode's own shape — the arrow, the ring, the box — so which
of the three is in force is readable without pressing anything, and its tooltip
names the three keys from the shortcut table rather than from a string, so a
rebound one is the one the interface reports.

The chip is live only where nothing smaller owns the widget — a cage that is
up, a curve being authored and a selected object each already have it — and
greyed with the reason on it rather than taken away, because a chip that came
and went at the head of the bar would shift every slider beside it.

Its arms follow the one rule every manipulator here follows now — a share of
the camera's distance, so the widget is the same size to the hand at every zoom
(see *The manipulator on an object*). It was sized to the subtool's own bounds
once, because the widget was depth-tested and a fixed reach drew nothing on a
form it sat in the middle of; the manipulator is drawn over the clay now, so
that reason is gone, and a widget that grew with the subtool was off the screen
at any zoom that showed the subtool's detail.

The pivot is the layer's transform. For a subtool standing where its layer
transform puts it that is its middle; a layer whose geometry was built
off-centre inside it carries the widget at the layer's origin rather than at
the form's, which is worth knowing before reaching for it.

**Choosing a mode is entering a mode.** While the whole-subtool manipulator is
up, a press on the clay that misses a handle is the mode's free gesture — the
centre's, which slides in the view plane or scales uniformly, or the outer
ring's, which turns in the screen plane — and not a stroke; the arrows are for
the constrained gesture, the form itself is the free one, as in ZBrush. The
brush ring is not drawn meanwhile, since it would promise the wrong thing. Off
the form a press still orbits, so the model can be turned to look at without
leaving the mode; pressing the chip already in force leaves it.

**And reaching for a brush leaves it.** Choosing a brush — from the shelf, or
with the mask key — puts the whole-subtool manipulator away, because it is a
mode and the sculptor has just said what they mean to do. Without it the next
press dragged the subtool with nothing on screen having changed, which is the
worst kind of surprise. A *selected object* keeps its own manipulator: choosing
a brush does not unselect what is placed, and that widget follows the selection
rather than the mode.

**A stroke lands on the subtool where it is drawn.** A field layer's transform
moves what the tape evaluates, so the form is drawn and picked where the
manipulator put it — while the stamps a stroke deposits go into the layer's own
frame, which the transform then moves *again*. Measured: a subtool dragged
three units along X was sculpted three units past the pointer, and the surface
under the brush never moved at all. So the gesture is carried back into the
layer's frame before anything is derived from it, and the brush radius with it
— the conversion a carried mesh has always made, now in the one place both
field routes pass through, so the baked verbs and the stamping ones cannot
disagree about where a stroke landed. The mirror follows, and rightly:
reflected in the layer's own frame, symmetry is about the subtool's axis rather
than the world's, which is exactly what the engine's layer mirror does to the
items on the stamping route.

**And a mask freezes the surface it was painted on.** The same root at the
other end of it. A mask belongs to its layer and every consumer reads it where
the layer's own content is — the gate on a stamp, the engine's stroke mask, the
mesh sculptor — while the brush painted it where the form is *drawn*. On a
moved subtool those are two places, so the frozen region sat beside the form it
protected and the freeze quietly did nothing. Painting is carried into the
layer's frame, and the readback the viewport uses to draw the frozen region is
carried the other way, so what is drawn and what protects are the same cells.
`subtools` holds all three: the stroke, the mirror and the mask, each measured
against an unmoved dab so a threshold cannot pass on a stroke that stopped
working for some other reason.

**A grid moves with its subtool, and the host is what moves it.** ClayCore
holds the placement for a voxel layer and honours it wherever the *document*
answers — `clay_layer_bounds` on a grid moved three units along X reports
`2.92…3.16` where it reported `-0.08…0.16`, composed exactly as the SDF arm
composes it (ABI 0.52.3, ClayCore
[#318](https://github.com/CyberdyneCorp/ClayCore/issues/318)). What the grid
API has no room for is a placement of its own: `clay_voxel_grid_create` takes a
cell size and nothing else, `clay_voxel_raycast` answers in the grid's own
coordinates, and the chunk mesher hands back grid-space vertices. That is the
same arrangement a carried mesh has, and it is composed the same way — in
`append_voxel_layer` on the way out, in `pick_active_grid` on the way in (the
ray carried into the cells and the hit carried back), in `stroke_voxel` for the
gesture and the brush radius, and in `layer_bounds`, which places the measured
box the manipulator and Frame All both read. Until it was, the whole-subtool
manipulator on a voxel subtool moved the widget and left the form standing —
precisely the gap a carried mesh had before the host began applying the
placement itself.

**And the mirror stands where the mirror is.** The engine reflects a layer's
items through the plane where that layer's own coordinate is zero — "the layer
transform moves the plane with the layer" — so a mirrored stroke on a moved
subtool answers across the *subtool's* axis. The brush ring and the plane
overlay were reflected and drawn through the world's instead, which put the
mirrored ring a whole displacement away from the dab it promised and the orange
wall somewhere nothing would land. Both take the active subtool's frame now,
and the arithmetic they reflect through is the model's own `Transform` — the
same `into_world`/`into_local` pair the engine adapter carries a stroke with,
so the ring, the plane and the dab cannot drift apart. Sculpting a grid stays *coherent* while that is
true, because what is drawn, what a ray picks and where a dab lands are all the
same unmoved cells; what is missing is the move. The mask follows the same
rule, and knows which case it is in — carried into the layer's frame on a field
or a mesh, left alone on a grid, where carrying it would push the frozen region
off the cells it protects. `subtools` measures the invariant that holds
whatever the answer: a dab lands where the pointer found the surface, on a
moved mesh and on a grid alike.

**A moved subtool is re-meshed where it was as well as where it is.** The
refill after a layer transform marked only the bricks the layer now occupies,
so the surface it had just made stood where it had been: the arrow was
dragged, nothing moved on screen, and the next stroke re-meshed a handful of
bricks around the pointer into a second form with holes in it beside the first.
The refill takes the union of the layer's bounds before and after now, the way
a moved object's already did. `visual_subtools` moves a subtool on the
viewport's incremental path and holds the picture to what a rebuild draws.

**Scale is uniform until the engine can carry three factors.** ZBrush's gizmo
scales per axis; here the axis boxes are absent in scale mode because
`clay_layer_set_transform` and the node transform take one `scale`, and an axis
handle would measure a stretch the engine cannot apply. Filed as ClayCore
[#373](https://github.com/CyberdyneCorp/ClayCore/issues/373); the handles come
back when it lands.

**A centre scale is metered from one arm's length.** A scale is a ratio of
distances from the pivot, and a press on the centre handle starts a hair from
it, so the ratio ran away in the first frame. The gesture is measured as if it
had started one arm out: pulling outward by an arm doubles the form, pushing
inward by an arm halves it — how ZBrush's scale reads, and what a hand can
meter. The refusal or substitution sentence stands in the viewport bar now,
beside the view chips; at the tail of the options bar it was past the right
edge at 1280 and read by nobody.

**One scale gesture is at most tenfold**, either way. The factor is a ratio
of distances from the pivot, and a press on the centre handle starts a hair
from it, so one pull to the edge of the screen was a hundred times — a form the
field's cache cannot track and nothing a hand meant. Ten times a drag is still
a big move; more is another drag. And where the cache still refuses the region
a transform would need, the transform is **put back** and the refusal shown,
rather than the field standing where the picture cannot follow: the manipulator
keeps following the hand, the clay stays at the last size the cache accepted.

And the viewport re-meshes on every frame of a manipulator drag on clay. The
drag's commands are not document edits in the ViewModel's accounting — they
were filed beside the cage's, whose drag moves control points and not the
surface — so nothing asked the viewport to look again: the field moved under a
picture that did not, and a stroke aimed by a ray through the moved field
landed beside the drawn form. A drag on a placed object or a whole subtool now
re-meshes what it dirtied as it goes, and marks the document unsaved once, when
the gesture ends.

**The three modes are one row of chips wherever the widget can be worked** —
under the object list, in the shapes panel beside the selected object, and in
the cage section — each chip carrying the shape its handle has in the viewport:
an arrow for Mover, a ring for Girar, a box for Escalar. Until that row stood
under the object list, the modes could only be changed with a cage up, so an
object's manipulator moved and did nothing else. The row is absent with
nothing selected, rather than drawn and inert.

**The widget is drawn over the clay, not in it.** A manipulator sits on the
middle of what it moves, and the middle of a placed sphere is inside the
sphere; depth-tested, it was three arrow tips poking out of the form and
nothing to grab, and on a small object inside a large one it was nothing at
all. The cage, the curve's control polygon, an object's outline and the
manipulator are all scaffolding around the clay, and scaffolding the clay hides
is not scaffolding — so the overlay reads no depth, and every handle is where
the hand expects it whichever side of the surface it is on. The strokes are laid
down three deep, stepped across themselves in the screen plane, so a handle is
a handle and not a one-pixel hairline over a shaded form — and the parts a
hand actually grabs are solid: a cone caps each move arrow, a block sits at
each scale handle and at the pivot, each face shaded from a fixed upper-left
light so it reads as a thing with sides. Rings stay lines; a solid on a ring
would be a fourth thing to grab.

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

### Crossing a layer from its own row

The representation bar's cards cross the *active* layer. A sculptor looking at a
stack of eleven subtools means the row they opened the menu on, so a layer's own
menu offers the crossings **that layer's** representation has — read from the
row rather than from the active layer, which is why a mesh row offers Voxels and
Campo (SDF) while an SDF row offers Voxels and Malha, in the same stack, at the
same moment.

Invoking one makes that row's layer active before aiming the conversion,
because the conversion acts on the active layer and a crossing asked of another
row would otherwise convert something else entirely.

It is aimed **in place**: the source leaves as the result arrives and the result
stands where it stood, which is what a sculptor means by converting *this*
layer. That setting had been in the domain from the beginning, with nothing in
the interface able to ask for it.

The entry carries an ellipsis and opens the conversion panel rather than
converting on the click. A crossing costs work, a crossing into cells needs a
size chosen, and one that would not fit the budget is refused — the panel is
where all three are said, and it is the same reason the bar's cards are inert.

### Showing one subtool alone

A layer row's own menu offers **Mostrar só esta**, which hides every other
subtool, and **Mostrar todas**, which brings back the visibility each of them
had — not "all visible": a subtool the sculptor had hidden before soloing is
hidden again afterwards. Soloing a second subtool while one is already alone
still restores the pattern that stood before the *first* solo. The entry says
which of the two it will do, read from the scene rather than from the eyes in
the stack, because a row's eye cannot tell a solo from someone who hid three
layers by hand.

**Solo changes nothing else.** It does not move the active subtool — a solo
elsewhere hides the one the brush is pointed at, and the stroke is refused with
that reason rather than landing where nobody can see it — and it adds nothing
to the undo history: solo, release, ⌘Z takes back the last *edit*. That is not
free. The engine has no journal pause: once undo is enabled every command is
recorded, `SetLayerVisibleCmd` among them, and the merged SDF surface cannot
drop a layer any way other than engine visibility. So the commands are made and
then **stepped over** — undo hops the entries a solo left, the same way it
already interleaves mesh gestures and crossings with the engine's own stack —
and the history panel's depth is reported without them.

The hide-and-restore is a primitive rather than solo's private business,
because baking one subtool alone is the same operation: hide the others, run
something, put the flags back. The restore owns the exit, including the failing
one — an operation that refuses halfway leaves the sculptor's scene exactly as
it was, and its own commands are not left for anyone to undo.

**Saving while soloed writes the real pattern.** A solo is a way of looking at
the document, not part of it, so the file gets the visibility the sculptor set,
and the solo is put back around the write. A reopened or crash-recovered
document shows what they set and is not soloed.

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

### A boolean between two subtools

*Arquivo → Booleana entre subtools* — beside **Formas**, because it is the
other half of putting forms in a scene — combines two whole forms into a third.
Pick the two, pick **União**, **Subtração** or **Interseção**, read what it
costs, and confirm. What arrives is a subtool like any other: active on
arrival, sculptable, movable with the whole-subtool manipulator, and available
as an operand again. It stands in the right region under the shapes, for the
same reason the shapes do: a window over the viewport covered the two forms
being cut from one another. The rail reaches it beside **Formas**.

**The panel names the two roles rather than numbering them.** *Base — o subtool
que é cortado* and *Ferramenta — o subtool que corta*, because subtraction is
not symmetric and "A menos B" is the whole of what is being chosen; with a
subtraction set up the panel prints that sentence with the two names in it, and
swapping them changes it.

**It is a resolved boolean, not a live one, and the interface says so.** The
engine composes the layers of a document by hard union
(`clay/scene/tape.h`) — a live layer-level boolean is ClayCore
[#321](https://github.com/CyberdyneCorp/ClayCore/issues/321), filed and open —
so what this does is sample each operand into a volume and combine the two in a
new layer. Moving an operand afterwards does not update the result.

**Which is why the operands are kept.** They stay in the scene, hidden, and one
⌘Z takes the whole operation back with them visible again exactly as they were.
A sculptor who can still reach the cylinder can move it and run the boolean
again; one whose cylinder was consumed cannot. *Consumir os operandos* removes
them instead, and the panel says what that costs before it runs.

**The cost is stated first, in the same vocabulary as the crossings**, because
it is the same kind of crossing: the surface moves by half a cell, features
thinner than a cell vanish, sharp edges become staircases at the cell size, and
the parametric edit lists of both operands are spent. The resolution defaults
to the finer of the two operands' own detail — a grid says what it is worked
at — and is then the sculptor's to change, with the figures following the
slider. **Nothing runs unconfirmed**, and until two different subtools are
chosen there is nothing to press.

**Every representation can be an operand**, with the crossing each one needs
performed as part of the operation rather than demanded beforehand: a field is
sampled out of the document with the rest of the scene hidden, a grid is read
back through `clay_item_volume_from_voxels`, and a mesh takes the same crossing
placing one as an operand pays. Both operands are sampled over **one** region —
the pair's box, padded by the band — so that the two halves of the result sit on
the same lattice and meet cell-for-cell at the join, and so that the cost stated
beforehand is the cost of what was actually done. Outside the lattice a volume
item reads as *outside*, which is what lets a grid operand — which has no region
to be given — take part in an intersection at all.

**A boolean that cannot be run says why, and names which subtool.** An empty
operand, a ghosted or locked one, a pair whose region does not fit the
document's memory budget, and an intersection of two forms that do not meet —
which is nothing, so it is refused rather than made into an empty subtool. The
scene is left exactly as it was in every one of those cases, the borrowed
visibility included.

## When a subtool has become costly to evaluate

A chain of edits steepens the field it produces: each bake resamples what the
last one left, until a ray march has to take many small steps and every dab
pays for it. The engine measures that — it reports a *safe step scale*, which
falls as the field steepens — and says when collapsing the layer into one
volume is worth it. Measured here, a layer the engine advised on took a dab
from **56 ms to 13 ms** once collapsed, and collapsing it took about six
seconds.

So the subtool panel says so, and offers the one thing that helps. It appears
only while the engine is advising it, and nothing is collapsed until the
sculptor asks: it costs seconds and it changes what the layer holds, which is
not a decision to take on someone's behalf while they are working.

**Asking is free and acting is not, and neither is estimating.** The advice
costs **33 µs** on a 97-item layer; estimating what the collapsed layer would
occupy costs **287 ms**. They used to be one call, which is most of why the
advice never reached the screen — nothing could afford to ask on a refresh
path. The scene carries the advice; the estimate is asked for where a sculptor
is deciding.

## When a mesh has stopped taking detail

The field layer's answer to a form that has become expensive is to collapse it;
the mesh layer's answer to a form that has become *wrong* is to rebuild it.
**Refazer a malha** — DynaMesh, in the vocabulary most sculptors bring with
them — throws the layer's geometry into a voxel grid and marches a new surface
out of it. Overlapping shells fuse into one skin, self-intersections resolve,
triangles stretched thin by a long pull disappear, and the density comes out
even across the whole form. It is what you reach for after pulling a limb
somewhere its triangles could not follow.

It sits under the layer stack, where a field layer's *Otimizar* row sits, and
it is offered whenever a mesh subtool is active rather than waiting for advice.
That difference is deliberate. A field's steepening is something the engine
measures and can raise a hand about; there is no equivalent number for "this
topology has stopped taking detail" — the sculptor is the one who can see it,
so the control waits for them instead of waiting for a measurement that does
not exist.

**One number and three switches.** The *Resolução* is cells across the form's
longest dimension, so it means the same thing on a thumbnail and on a bust, and
the engine reports back what it came to in world units. Detail finer than a
cell does not survive, which is the whole of what choosing it decides. The
switches are *Remover pedaços soltos*, which discards fragments too small for
the resolution to have described anyway; *Seguir a forma atual*, which pulls
the new surface most of the way back onto the one it replaces so the sampling's
rounding is recovered — a lerp and never a snap, because pulling all the way
back reintroduces the geometry the rebuild was asked to remove; and *Arestas
vivas*, which holds corners instead of rounding them at the cost of the
watertight guarantee. The engine marks that last mode experimental and so does
the hint on it.

**It says what it destroyed.** Every rebuild is destructive — vertex and polygon
identity are gone, and texture coordinates are dropped rather than reprojected,
because a UV layout spatially reprojected across a seam is a stretched one that
looks preserved. So the triangle counts before and after stay beside the button
until the next rebuild, along with the number of separate pieces the form is now
in. That last one is the answer to the question a sculptor actually asks after
looking at the result: *did those two actually join?*

**One undo step, and a refusal costs nothing.** Nothing is written until the
rebuild has succeeded and validated, so asking for a resolution the form turns
out not to survive gives a sentence and leaves the layer byte-identical. That
is what makes a resolution control safe to offer at all, rather than something
to guess at and then undo.

**Sculpting works immediately afterwards, and after undoing it.** A mesh
sculptor is a weld and an adjacency pass over every triangle — 160 ms on the
reference form — and a rebuild replaces every triangle it was built over. The
one held over the old geometry is dropped before the rebuild and a new one is
ready by the time the panel updates, so the next stroke lands rather than the
press orbiting the camera. It also holds through **undo**, which took its own
work: the engine's geometry revision is documented as moving whenever a layer's
triangles are replaced wholesale, and measured on 0.73.0 it does not move when
*history* replaces them — 1, 2, 2, 2 across attach, rebuild, undo, redo, while
the triangle count goes 119,100 / 37,752 / 119,100 / 37,752. So the application
keeps its own note of where each rebuild sits in the history. Without it, a
sculptor who rebuilds, dislikes the result, undoes and carries on gets *the mesh
changed its vertex or index count under this sculptor* on their next dab.

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

**A crossing adds a layer, or replaces the one it read.** Adding is the
default because it cannot lose work: the source stays, and a sculptor who
dislikes the result removes the layer it made. **Substituir a camada** is what
a sculptor means by converting *this* layer — the source leaves as the result
arrives and the result takes its row in the stack, rather than leaving a pile
of supplanted originals nobody meant to keep.

Either way it is **one undo**. The result keeps its derived name — `Forma ·
voxel` rather than `Forma` — because that name says what the layer now holds,
and because a voxel grid is reachable only by name (ClayCore
[#365](https://github.com/CyberdyneCorp/ClayCore/issues/365)): handing the
result the source's name would put two layers through one grid for as long as
an undo kept both in the document.

The removal and the reorder are engine entries of their own — a group does not
swallow them — so the crossing records how many it left and steps over all of
them together, and the depth the interface reports discounts the extras. That
is the same shape the solo entries already have: a sculptor made one crossing
and has one thing to take back. The panel used to say a crossing could not be
undone at all, which stopped being true when crossing undo landed and would
have been the worst place to be out of date, standing as it does beside a
control that removes a layer.

**And where each carried layer stands.** A layer transform moves no vertex in
the engine and touches no grid — the placement is applied on the way out, as
the carried mesh is read — so the number sat still while a whole mesh subtool
was being dragged: the manipulator moved and the form did not, and a mesh
subtool could not be transformed from the application at all. The field side
has no such gate; its surface is re-meshed from the bricks the move dirtied,
which is why this only ever showed on a mesh or a grid.

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
- **A rig belongs to the subtool that holds its nodes.** A document may carry
  one per subtool: activating a subtool presents its rig as it was left, rigs
  on other subtools are untouched by what you do to this one, and a subtool
  carrying none says so rather than handing you a neighbour's. All of them are
  read back when a document is reopened, so a scene with two rigs comes back
  with two.
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

**The symmetry plane is an outline, not a lattice.** Drawn as a grid across
the mirror plane it was a wall of orange lines over the form on a running
build, whatever the dimming, because the camera sits inside the plane's extent.
It is six lines now — the plane's edge and its two centre lines, the mirror's
axis where it meets the floor — at a fifth of the accent: it says where the
mirror is and puts nothing across the clay. The navigation gizmo in the corner
draws each half-axis as a bundle of five lines, so it reads as a rod rather
than a hairline.

**A surface the device cannot hold is drawn coarser, not fatally.** A subtool
scaled up a few times is ten million vertices at the field's fixed resolution.
The renderer asked for the downlevel default of 256 MB per buffer, so the
vertex buffer for that surface was a validation error in `create_buffer`, and
wgpu's default handler ends the process on one — a scale gesture closed the
application. The device is asked for the adapter's own ceiling now (gigabytes
on a desktop), a validation error is reported rather than fatal, and a layout
that would still not fit is refused with the picture left as it was and the
viewport dropped to the coarse level, which the geometry panel says
(*detalhe reduzido*). A brick with nothing in it comes back from the engine as
a mesh with no attributes, which the shading pass used to report as a failure
every frame; it is read as empty.

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
  flank from the bottom of a fold; three passes after the scene reduce the
  depth it wrote to half resolution, sample a hemisphere around each pixel
  there, and bring the result back with a filter that weighs each neighbour by
  how near its depth is — so the shading stops at a silhouette instead of
  bleeding across it. Depth rather than the vertex normal because the reference
  form is about seven triangles per covered pixel, where a screen derivative of
  the normal reports the tessellation instead of the shape. 0.15 ms a frame at
  1080p and 0.54 ms at 4K. Its radius is a fraction of the form's own size, so
  an import at any scale is shaded the same way; it runs at every sample count,
  including one.
- **Cavity shading** (*Vista → Realce de cavidades*), on by default and off
  under the pen. Occlusion works at the scale of its own radius and says
  nothing about a crease finer than that, which is most of the detail in a
  finished sculpt; this reads the curvature of the reconstructed surface and
  darkens where it turns into itself.
- **Studio lighting** (*Vista → Iluminação de estúdio*), offered beside MatCap
  and never in place of it. A MatCap's lighting is welded to the camera —
  orbiting the form orbits the light with it — which is what makes it good for
  reading form and useless for judging how a surface takes a real light. The
  studio rig is three lights fixed in the *world* with a filmic curve over
  them, and its key light casts through a shadow map fitted to the form, so a
  fold shadows itself. The highlight travels fourteen times as far across the
  form under the same orbit. The shadow can be switched off
  (*Vista → Sombra do estúdio*); nothing is allocated for any of it until the
  rig is first asked for.
- Multisampling on the scene, four samples by default and resolved down to what
  the device will actually take. The interface is drawn into the resolved
  target afterwards rather than multisampled with it: text and panel edges are
  already laid out on the pixel grid. A device that will not multisample the
  surface format gets a post-process pass over the silhouette instead, which is
  never run alongside multisampling — four samples and a blur over the top is
  paying twice to lose detail once. Measured at 1080p against a 16.7 ms budget:

  | | ms |
  |---|---:|
  | one sample, with the post-process pass | 0.26 |
  | two samples | 0.24 |
  | four samples | 0.32 |

  Which says something worth knowing: on this card the fallback costs *more*
  than two samples of real multisampling and reads worse, so it is what a
  device gets when it has no choice rather than a cheaper setting to reach for.
- **Reversed depth**, with a near and far plane derived from the viewing
  distance and the size of what is on screen rather than fixed. A form far
  smaller than the old fixed near plane of 0.01 was clipped away when zoomed
  into; a large one spent the whole useful precision of its depth buffer on the
  first hundredth of the range.
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
  the surface rather than hanging at an arbitrary depth. Drawn as a ribbon a
  couple of pixels wide rather than as a line: WebGPU has no line width, so a
  line list is one pixel whatever the display and has no coverage for the
  scene's multisampling to resolve.
- **Quality follows the pointer.** A frame drawn under a moving pen is one of
  hundreds in a stroke and no brush decision rests on the quality of its
  occlusion, so the sample count drops while a gesture is in progress and the
  cavity term is switched off. It rises again 160 ms after the pointer stops
  and fully after 600 — not on release, because the gap between two dabs of one
  stroke would otherwise be paid at full price, which puts the cost exactly
  where the latency is measured.
- **Per-pass GPU time** in the diagnostics window, measured with the device's
  own clock rather than inferred from how long submission took, alongside the
  occlusion resolution, the draw calls, the triangles and the bytes uploaded. A
  device without timestamp queries says so and renders unchanged.
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

**What a running English build still said in Portuguese is translated.** A
screenshot of the live application, not a capture, showed Perspectiva, Frontal,
Lateral and Superior under the viewport, Dura and Suave on the edge chips, and
"ainda não gerado" in the geometry panel: each was drawn from the domain's own
`label()` rather than from the string table. The four views, the four edge
profiles, the three reference planes, the curve's joins and profiles, the six
mask operations and the two detail notes are in the table now, in all three
languages; the shell's untranslated-label ratchet (`LABELS_STILL_DRAWN`) stands
at ten, all of them identifiers — SDF, mm, a mesh's own name — rather than
words.

**The untitled document is named in the interface's language.** A fresh
document is "Sem título" to the document ViewModel, which knows no locale, so
that Portuguese reached the menu bar's document label and the file name the
save and export dialogs offered on an English or Spanish build. The ViewModel
still stores and resets to that one marker — it is `clayspace_vm::UNTITLED`
now, spelled out nowhere else — and the shell translates it where the name is
shown: **Sem título**, **Untitled**, **Sin título**. A name that came from a
file passes through untouched.

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

**The brush names are translated.** All twenty-one, on all three representations —
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

**The chrome clears away.** Tab, or Janela → Modo foco, hides the tool rail,
the options bar, the representation bar, both inspectors, the shelf and the
status area, and leaves the sculpt. The menu bar stays, because a mode nobody
can find their way out of is worse than no mode and Tab is not discoverable from
an empty window; and a floating readout keeps the brush's ball, its name, the
representation a stroke would land on, and its size, intensity and flow, since
hiding the bar that carries those without replacing them would be focus in name
only.

It is a presentation override rather than a layout: it hides the regions without
touching the sizes and collapse states a sculptor chose, so leaving it puts
everything back as it was, and it is deliberately not remembered — an
application that opened with its panels gone would look broken.

**The stroke's settings are on one bar.** Intensidade, Tamanho, Fluxo,
Suavização and the symmetry axes, with an engaged axis in the soft accent rather
than a raised grey: symmetry is state a sculptor needs to see without looking
for it, and a mirrored stroke they did not expect is the most expensive surprise
on the bar. The smoothing and the symmetry moved here from a right-panel section
and a left-panel one — moved rather than copied, and the left region's
sculpt-settings section held nothing else and is gone with it. The edge profile
was moved too and put back: the bar overflowed and scrolled the colour swatch
out of view, and an edge is chosen occasionally where a smoothing is dialled
while drawing.

**Brushes can be starred.** A brush's own menu adds it to a shortlist, the
shelf's filter column offers ★ Favoritos beside the representations, and the
list is kept between sessions. It spans every representation, because its
purpose is finding a brush again rather than describing the active layer — one
the active layer cannot run is listed dim and refuses the click, like any brush
met while browsing.

**The viewport's quality profile is remembered** too.

**The regions move, and are remembered.** The left region, the right region and
the shelf are resizable, clamped so that none can vanish or swallow the
viewport; each can be put away and brought back from the Janela menu, with a
reset that returns every one to the design's own size; and the arrangement is
stored beside the recent documents and the chosen locale, so it is as it was
left when the application opens again.

That had been a stated requirement — *"The user SHALL be able to resize and
collapse each panel region and restore the default layout in one action. Layout
SHALL persist across sessions"* — with none of it true. The regions were drawn
at fixed widths, the Janela menu was declared and left empty, and the `layout`
module carrying the sizes, the bounds, the collapse state, a reset and a pair of
serialisers was exported to no consumer at all. Its own tests passed throughout.

A collapsed region draws nothing and gives its space to the viewport, and keeps
the width it had so that bringing it back returns the size a sculptor chose. A
corrupt stored line costs the arrangement and never the start-up.

The regions from the design: a menu bar, a tool options bar, a left region with
the scene tree, layer stack and sculpt settings, a representation bar and the
central viewport under it, a right region with material, geometry and brush
inspectors — and, while either is up, the shapes and boolean sections — a brush
shelf, and a status area with a memory meter, the active backend and the
working unit.

**The three representations stand above the viewport, as equals.** One card
each: an icon of a distinct shape, the representation's name, and a phrase
saying what it is. The active one is raised and railed, in the same grammar the
active layer row wears. The other two are shown rather than hidden, because the
point of the bar is that a sculptor can see what the alternatives are — the
interface used to say this in a three-letter tag on a layer row and a line of
text at the far end of the viewport bar, and that line was half untranslated,
drawing the engine's own word under a translated prefix.

**A card converts nothing.** Crossing between representations costs work and is
not always reversible, so it stays behind the conversion panel where the cost
is stated and confirmed. The crossings are a row beside the cards, and they are
the ones the domain actually declares from the active representation rather
than a written list — invoking one aims the panel and opens it, and a panel
already open is aimed rather than closed.

**The manipulator keeps its size, and says what the numbers are.** Its arms are
a share of the camera's distance, so it stays the same size to the hand whether
the sculptor is looking at the whole scene or has zoomed into a pore — true of
a placed object and a whole subtool since they were built, and true of a
deformation cage now: the cage's was a share of the cage alone, so it shrank
with the camera while an identical-looking widget on the object beside it held
still.

While a manipulator is on a placed object, its transform stands in the
viewport's lower-leading corner: position, rotation, the axis that rotation is
about, and scale. A widget shows that something moved and never by how much,
which is the first question asked when two objects have to line up — and
nothing in the interface reported a placed object's position at all. Shown for
a placed object alone: a cage's target is a set of control points and a layer's
is everything it holds, and neither has one position to report.

An axis and one angle for rotation, because that is what the engine's ABI
speaks — it stores a quaternion and calls the axis and angle *a* representative
rather than the representation, so three Euler angles would be one of several
answers and would change when nothing had moved.

**A placed object stretches per axis.** The manipulator's three scale boxes are
offered on it: a box on an axis stretches that axis, the centre handle takes
all three. A whole subtool still scales uniformly, because the engine's *layer*
transform takes one factor where its *node* transform takes three — the handles
are offered exactly where they can be applied.

They were hidden on everything but a deformation cage for as long as nothing
had bound `clay_layer_set_transform_nonuniform`, which the engine has carried
since ABI 0.54.0 against a pin of 0.60.0. The belief that "every transform in
the engine's interface takes a single scale factor" was written into the
domain, the manipulator, the readout and the specification, and nothing went
back to check it. A capsule could not be squashed into a slot.

What a stretch costs is not what one would guess: the field stays 1-Lipschitz,
so the safe step scale is unchanged and a marcher takes the steps it always
did. What is lost is exactness — the value becomes a bound on the distance,
short by at most the ratio of the largest axis to the smallest and never an
overestimate — which matters to a consumer reading it *as* a distance and to
nothing else. A uniform value compiles to identical tape.

**The floor dissolves rather than ending.** The grid fades out before it
reaches its own extent, so it draws no rectangle around the scene, and each
line is cut into segments so the fade varies along it — a line drawn as two
vertices takes the interpolation between its ends, and both ends of a grid line
are equally far from the middle, so a per-line fade dissolves nothing. Every
fifth line is major, with the two axes strongest, so a distance can be counted
rather than estimated.

The fade is by distance from the origin and not from the camera. Overlay
geometry is uploaded when the overlays change rather than every frame, so a
camera-dependent colour would rebuild and re-upload the whole grid on every
orbit; and the form sits at the origin, so a grid densest beneath the sculpt
says the same thing from every camera instead of changing when the sculptor
zooms.

**The viewport's quality profile is chosen from the Vista menu** — Desempenho,
Escultura, Apresentação — beside the shading terms it belongs with. The three
tiers and the stroke-time degradation between them have existed since the
renderer was written, and nothing in the application had ever selected one: the
governor was built with the default and never told otherwise. Choosing a
profile changes what an *idle* frame is drawn with and never what is drawn, so
it emits no command and enters no history. It is not remembered across
launches, for the same reason the shelf has no favourites.

**The inspector answers what is being sculpted**, in one section that keeps its
place while its contents change: a field states how many items its edit list
holds and whether it has been collapsed, a grid its cell size, how many cells
hold anything, and how it is drawn, a mesh
the contract its brushes work under — they move the vertices that are there and
neither add nor remove any, which is the reason Inflar and Suavizar behave
differently on a mesh than on a field. Each is headed by the representation it
describes. The grid's controls used to stand under `GEOMETRIA`, which is also
the polygon counts' heading, and a fold is keyed by that word: putting one away
put the other away too.

Only what the domain holds. Every per-layer field quality, evaluation
resolution, surface offset, voxel size, grid bounds, normals control and
subdivision level the concept depicts is absent, because none of them is a
value this application or the pinned engine can express for a layer — a control
for something nothing reads is an interface that lies about what the program
does. Three things that do exist stay where they are rather than being
duplicated: the combine vocabulary belongs to the stroke and stands in the
options bar, a grid's recorded passes are nested under the layer they were
recorded on, and the offer to collapse a costly field appears under the layer
list only while the engine is advising it.

As the window narrows the bar gives up its phrases first, into the tooltip,
then its heading, and never its crossings. A card always keeps its icon *and*
its name: below about five hundred pixels of central region the bar scrolls,
because a representation told by shape alone is exactly what the contrast tests
elsewhere refuse to allow.

The **accent marks active state**, at the scale of a rail, a ring or a label
and never as a fill: the active brush wears a ring and an accented name, the
layer being sculpted wears a two-pixel rail at its leading edge, and a slider's
travelled range is drawn in it. It marks nothing else — no panel chrome, no
heading, no border, no hover. A test asserts the accent's coverage *in the
shelf* stays about constant as the active tool changes, so the brush mark
cannot quietly spread.

The rail is an addition to the tone step and not a replacement for it. The
active row is raised, railed, and set in primary rather than secondary text, so
covering the hue still leaves it identifiable — which the design requires and a
test checks. It earned a mark because tone alone could not carry it: `panel` to
`raised` is three and a half per cent of relative luminance, and it was the only
thing saying which of four subtools a dab would land on.
**The tool rail on the leading edge** — the region the design named and the
first build left empty — holds, as icons with their word and key on hover:
mask painting; frame, polyframe and the reference images; the shapes and
boolean sections, the deformation cage, the curve and the deformations; undo
and redo. Every
button dispatches the command its menu entry does under the same conditions —
the cage is grey with the reason on it where the layer cannot be caged, undo
is grey where there is nothing to undo — so the two cannot disagree. It exists
because the menus were the *only* way to the shapes panel, the cage, the
deformations, the references and the curve, and a panel three menus deep is a
panel a new sculptor never opens.

**The options bar is headed by what works the whole form.** The manipulator on
the whole layer — one chip, its mode on W, E and R — and **Deformar…** stand at the head
of the bar before a hairline rule and the sliders. Both are modes a sculptor
enters and leaves rather than amounts, and a mode that cannot be seen is the
most expensive thing this window can hide; the deformations chip is the same
toggle the tool rail and the Dinâmica menu push, so the three cannot disagree.

The active brush's badge stood here and does not now: the shelf along the
bottom already draws which brush is in hand, lit and named, and the same fact
twice on one screen is a row of pixels that says nothing new.

**And the bar ends inside the window.** It did not: the badge, a wide gap at
every group boundary and a size slider that spelled the same size twice —
`Tamanho · 1,8 mm` beside a readout of `0,180` — added up to a bar a hundred
and sixty pixels wider than the design's 1280, so Alpha stood off the right
edge in every language and the only clue was a cut word. The groups are told
apart by a hairline instead of by twenty pixels of air, each is as wide as its
longest word rather than as wide as it was drawn first, and the size slider
*reads* in the sculptor's unit while still editing engine units — one fact
once, in the widest control on the bar. A test measures the last group's right
edge against the window in all three languages, because the words are what
overflowed. A window narrower than the bar still scrolls it sideways rather
than cutting the last control off.

The **material preview is the material**: the MatCap is a picture of a lit
sphere, so the swatch shows that picture — the same recipe the viewport shades
with, cut out of its square — and Terracota reads warm and Polido reads shiny
before either is read. Clicking it cycles the materials, and says so on hover.
The view chips and the symmetry chips name their key on hover, so `1`–`4` and
`X` `Y` `Z` are learnt where they are used rather than from this page.

**A slider shows how far into its range it is.** One widget draws every slider
in the shell: a track in the ground's tone with the range already travelled
filled in the accent, a knob restrained at rest that lifts under the pointer,
and the value monospaced and right-aligned above it. The fill is state rather
than ornament — it says what the digits cannot say without being read, which is
what a sculptor adjusting Intensidade mid-stroke needs — so it spans the track's
start to the knob and a value at the bottom of its range draws none at all.
Before this the three sliders at the head of the options bar were a hairline
with a grey knob on it.

**The viewport is darker than the shell around it.** Four surface tones now,
ordered: the sculpting viewport, the application ground, a panel on it, and a
raised row. The viewport and the shell were one colour, so the sculpt sat on
exactly the tone the panels were built from and there was no edge between them
— the design draws no outline around the viewport, so the tone step is the only
boundary there is. The grid's two tones dropped with it, keeping the distance
above their own ground that they were tuned for rather than becoming more
prominent as a side effect.

**The shelf can be browsed.** A column at its leading edge holds Disponíveis
and one entry per representation: the first is the sculpt workflow, unchanged
and the default, and the others answer the question the representation bar
raises — a sculptor who can see that Voxels exists, and what crossing to it
would cost, still had no way to find out which brushes they would get short of
converting and looking. While browsing, a brush the active layer has no verb
for is drawn dim, says so on hover, and cannot be picked; a brush that could be
clicked to no effect is exactly what the shelf's absent-rather-than-disabled
rule exists to prevent. Which set is shown is interface state: it emits no
command and is forgotten when the application closes.

There is no favourites filter yet. It can go in the store the arrangement of
the regions now uses; it is simply not built.

The active swatch also stands on a raised backdrop, and a swatch lifts the same
way under the pointer — so the active brush is carried by tone as well as by
hue, which is what a colour-blind sculptor reads, and the shelf follows the
"quiet until addressed" rule in the one place it did not. The sections of a
panel are separated by a hairline rule in the separator tone, never by a box.

**Every section folds from its heading.** The right region carries up to ten
sections and the left has grown too, so the heading row is a control: a click
puts the body away and a second brings it back, with a chevron at the row's
trailing end — faint at rest, lit under the pointer — saying which way it
stands. A fold is interface state and not document state: it is kept in the
interface's own memory keyed by the heading's word, enters no history, emits no
command, and is forgotten when the application closes, so every section opens
shown. The two placing sections keep their `×` instead, which puts them away
altogether.

**The shapes and the boolean are docked, not floated.** Both were windows over
the viewport, and the viewport is where the form a shape is placed into, or cut
from, stands: each covered the thing it was being used on. They are sections
of the right region now, under the material and above the geometry, drawn
while their toggle holds and put away from the `×` on their heading, the rail
button or the menu entry — all of which push the same command, so a test that
finds the close mark by its section's word and clicks it sees the section gone
on the next frame. *Inserting a form* says what the two sections hold.

**The four edge profiles are one segmented row.** Dura, Linear, Suave and
Gaussiana were four chips that wrapped in English and Spanish, leaving Gaussian
alone on a second line that read as a second setting. The row is now a bar
given the width of its row, so it cannot wrap: each word takes what it measures
plus a tight pad and the rest is dealt out evenly, the chosen cell lifted from
a ground-toned track the way a slider's knob is, the others dim until hovered.
A test asserts, in every locale, that the four cells share one top inside the
right panel, that the unchosen ink sits clearly below the chosen, and that a
click still sets the falloff.

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
- Changing symmetry costs its own entry, because a layer's mirror is state the
  engine records. The entry lands inside the first stroke that uses the new
  setting rather than beside the toggle, so one undo spends it with the rest of
  that gesture; it is written only when it differs from what *that subtool* was
  last told, so a run of strokes at one setting costs nothing extra.
- Hiding or showing a layer by hand is an entry, and undoing one brings the
  eye back with it. **Solo is not**: the entries it makes are hopped rather
  than offered — see *Showing one subtool alone* for why they have to be made
  at all.
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

Shortcuts are fixed. See [roadmap.md](roadmap.md). Panels *can* be resized and
collapsed — see [Interface](#interface); this line said otherwise until
`the-regions-move-and-are-remembered` built it.

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

**Colour on a field.** There is a brush colour now — see [Brush
colour](#brush-colour) — and it reaches the two representations that can carry
one. The SDF combine list still leaves `Pintar` out, and now for one reason
rather than two: `Op::Paint` is a real engine operation, but the brick cache
meshes the surface with colours off and nothing in the surface path carries a
colour to the GPU, so what it wrote would not be drawn. The operation stays in
the vocabulary so the mapping onto the engine is complete.

**SDF Pinçar.** `CLAY_DEFORM_MAGNIFY` is the field's pinch and magnify, one
signed strength, and it is *per item and local* — the engine says so in the
same paragraph that warns against wiring Move to `grab`: on a form blended from
several items, magnifying one pulls its share and leaves the rest behind. The C
ABI has an assembled-surface resolver for the drag (`clay_layer_move_surface`)
and none for the radial scale, and reconstructing one host-side would put field
math in this application. Upstream first, as
[ClayCore#391](https://github.com/CyberdyneCorp/ClayCore/issues/391).

**Alpha stamps on an SDF stroke.** `clay_layer_apply_stroke` scales its item as
a template per stamp and the chain hung off it is not resolved into each
stamp's frame. Measured as the rise along a stroke, swept over the stamp's
centre: anywhere on or inside the 0.35 template — `[0, 0, 0]`, `[0, 0, 0.2]`,
`[0, 0, 0.35]`, every sensible place to put one — moves the surface by nothing,
while `[0, 0, 0.7]` and `[0, 0, 1.0]`, which mean nothing in that frame, lift
the whole path evenly instead of leaving the mark the stamp carries.
`claycore/tests/alpha_deformer.rs` measures it. Upstream, as
[ClayCore#392](https://github.com/CyberdyneCorp/ClayCore/issues/392).

**A live preview under a voxel drag.** A grab composes destructively — the same
total drag split into eight one-cell emissions moves nothing — so Mover on a
grid holds the whole gesture and lands at pointer-up rather than following the
pointer. [ClayCore#393](https://github.com/CyberdyneCorp/ClayCore/issues/393)
asks for the transactional shape the SDF drag already has.

**A voxel Vinco.** The engine documents DamStandard on a grid as a *recipe* —
a small-radius erode with tight falloff and dense spacing — rather than a
verb, and a preset that borrows a name is not worth a shelf row until somebody
has looked at what it draws.

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

It stays here, and the subtool boolean did not move it. A **mesh subtool is a
legal operand** of *Booleana entre subtools* — the specification asks for the
crossing each representation needs to be "performed as part of the operation
rather than demanded of the sculptor beforehand", and it is: the layer's
triangles are sampled into a volume inside the operation —
`Document::mesh_layer_as_volume`, the same crossing that placing a mesh as an
operand already pays — and what comes out is a field. So what changed is who
performs the crossing and when, not whether one is performed. The vertices are
still quantised, the edge loops are still dropped, and the mesh layer itself is
left exactly as it was and stays sculptable.

The same is true of the older route, which also stands: choosing an imported
model in the shapes panel places it as a boolean operand and states the
crossing's costs first — the same figures the conversion panel states, because
it is the same crossing. So the *surface* still does not compose, in either
place, and a model can be used as though it did, at a price that is shown
before it is paid.

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

**A mask gating an operation** — *no longer absent.* It was, for as long as
`clay_item_set_gate` was accepted and inert: measured on 0.39.0 with a mask
sampling 1.0 at the cut's own centre and 65,752 cells painted, at every width
and threshold tried, a subtraction ate the protected region and the call never
refused. The wrapper matched the documented contract and the application
declined to make the call, because a call per stroke that does nothing is a
cost with no benefit and a promise the interface could not keep, and both
`mask_gate` tests were written to fail the day the engine honoured it. They
fired on v0.73.0. See *Masking* for what it does now.

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
| The coarse surface, while distant | Faintly speckled, and missing the mips it cannot have. The specks are degenerate triangles shading to a garbage normal — 316 of 86,130 on the reference form — which only shows because level 1 refuses gradient normals and forces face ones. Full-resolution SDF edits now use gradient normals immediately and do not share this artifact. Separately, a mip needs all eight children evaluated and the cache only evaluates surface bricks, so coarse blocks on the edge of the surface band never get one (70 of 242 here). Splicing full-resolution geometry into those gaps was measured at 0.69% of the frame and not taken — mixing spacings in one surface risks cracks where they meet | the engine's design, and ours |
| Suavizar, Relaxar | Subtle. Relax moves the surface by less than a cell per pass and the cell is 0.02, so they take the high-frequency edge off rather than removing a dent | the engine's design |
