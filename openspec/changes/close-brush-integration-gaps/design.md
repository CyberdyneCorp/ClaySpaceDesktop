# Design

## The colour is session state, not tool state

A brush's *size* belongs to the tool and the representation — this application
already stores it that way, and a sculptor who sets a small detail brush does
not want the blockout brush to shrink. A *colour* is the opposite: it is what
the sculptor is painting with right now, and it stays the same whichever
colour brush picks it up. ZBrush agrees, and so does every 2D application.

So the colour lives beside `CombineSettings` on the sculpt session — one
current value, plus a short list of recently used ones — rather than inside
`BrushSettings`, which is copied per tool per representation on every read.

`ToolKind::writes_colour` already names the two tools that consume it, so the
swatch is shown exactly where the value is read and nowhere else.

## Palette resolution is the engine adapter's job

A voxel grid stores palette *indices*; the colour lives in the palette. The
adapter therefore resolves a colour to an index before painting:

```text
brush colour -> find an existing palette entry within tolerance
             -> or add one
             -> paint with that index
```

Matching within a tolerance rather than exactly is what keeps a palette from
growing an entry per stroke when a colour picker returns values a float apart.
The engine caps a palette at 255 entries; past that the nearest existing entry
is used, which degrades to "the closest colour you already have" rather than
failing a stroke.

Structural deposits keep the neutral clay tone. Only Pintar paints with the
chosen colour, because "put material here" and "put *this colour* here" are
different instructions and ZBrush separates them the same way.

## The renderer was suppressing what it already computed

`MaterialUniform::tint.a` is the vertex-colour switch the MatCap and Studio
fragment stages both read, and the composition root wrote `false` into it on
every frame. Voxel and mesh vertices have carried real colours through
`sync_mesh_layers` all along; the SDF surface is meshed with `colors: false`
and every vertex comes back `[1, 1, 1]`, which is the identity under the
modulation. So the switch can simply be turned on: the field surface is
unchanged — to within the rasteriser's own precision rather than bit for bit,
since interpolating a constant colour is a ratio of two sums — and the two
representations that carry colour start showing it.

## Voxel Grab accumulates because the engine rounds

`clay_voxel_sculpt_grab` resamples occupancy nearest-cell and rounds per axis,
so a displacement under half a cell on every axis moves nothing — the engine's
own note says a drag fed raw pointer deltas "is dead until the caller
accumulates them past the voxel size".

Unlike the SDF drag, which takes a *total* displacement from a fixed anchor and
is idempotent, the voxel call translates occupancy destructively: two calls
compose. So the gesture holds what it has emitted, not what it has been asked
for:

```text
wanted   = current_world - anchor_world
pending  = wanted - emitted
if any axis of pending is at least one voxel:
    quantised = pending rounded to whole voxels
    grab(anchor_cell, quantised)
    emitted += quantised
```

Quantising before emitting rather than after is what keeps `emitted` equal to
what the grid actually did: rounding inside the engine and adding the unrounded
request here would drift by up to half a voxel per emission, and a slow drag
emits often.

The anchor is where the press landed, so the region dragged is the one under
the pointer when the gesture began, and it does not chase the pointer.

Under symmetry both the centre *and* the displacement are reflected, which the
existing `Mirror::point` / `Mirror::vector` pair already expresses.

## Voxel Planar is two-sided, and says so

`clay_voxel_sculpt_flatten` fills hollows below the plane as well as removing
material above it. The SDF and mesh sides of Planar are cut-only, which is what
makes a facet crisp. Faking cut-only on a grid would mean reading occupancy
back and reapplying it, which is host-side voxel math this application does not
do. So the tool is offered with the semantic the engine has and the tooltip
states the difference, which is the rule the rest of this application already
follows for representation-native verbs.

The plane's normal is the one voxel Scrape already uses — the mirrored up
vector — for the same reason: the engine takes a normal rather than deriving
one, and the two verbs should not disagree about where "the plane" is.

## Vinco and Argila override the combine operation

Padrão, Camada and Inflar take the operation from the Combinar panel: they are
the general strokes and the panel is what shapes them. Vinco and Argila are
*named brushes* whose whole definition is an operation and an accumulation, so
they set their own and ignore the panel — the same way Camada already forces
clamped accumulation whatever Acumular says.

```text
Vinco   -> Op::Incise, tight region, buildup, dense spacing
Argila  -> Op::Relief, buildup, dense spacing, clay profile
Camada  -> Op::Relief (panel), clamped        (unchanged)
```

Inverting Vinco gives `Op::Relief` — a ridge — which is what
`Combine::inverted` already says the pair are, and is a real ZBrush behaviour
rather than an invention.

Argila differs from Padrão by accumulation and spacing rather than by op,
because that is what differs in ZBrush too: ClayBuildup is Standard with
buildup on and a denser stroke. It differs from Camada by the opposite of the
same axis, which is what keeps the three distinct without a fourth engine verb.

## Move Topological bakes, so it is a region tool

`clay_item_volume_move_topological` takes an item carrying a volume and
re-samples it with the move applied — it is one of the baked field operations,
beside relax and flatten, not a deformer like the Euclidean drag. So it joins
`baked_stroke`'s family: the gesture is collected, the region is sampled from
the document, the operation is applied, and the result replaces the region.

That also settles its symmetry: like relax and flatten it is mirrored by
reflecting the gesture and running it again, because a layer mirror reflects
*items* and a baked volume is one item.

It is a separate tool rather than a modifier on Mover because the engine
documents them as different operations with different reach, and because a
modifier that silently changes which algorithm runs is exactly the kind of
hidden mode this application avoids.

## Masks: address the layer, never lend the handle

The C ABI is built for "a document and one of its masks, together". The Rust
wrapper lent the mask out as a `MaskRef<'doc>` and then asked for the document
mutably, which cannot be spelled. Nothing about that is the engine's fault, and
adding entry points to the C side would be fixing the wrong layer.

So the wrapper stops passing masks and starts passing *identities*:

```rust
pub enum MaskSource<'a> {
    /// No mask.
    None,
    /// A standalone mask the caller owns.
    Field(&'a MaskField),
    /// The mask attached to this layer of the document being edited.
    Layer(LayerId),
}
```

`Document::apply_stroke`, `relax_region`, `flatten_region`, `mask_extrude` and
the voxel-layer accessor take one. The resolution happens inside the wrapper,
where the raw document pointer and the raw mask pointer coexist for the length
of one C call and neither escapes — which is the arrangement the C side already
assumes, and `claycore` is the crate allowed to say so.

`ClayDocument::Layer::mask` then holds no geometry. What the interface needs —
whether a mask exists, and its painted extent for the mask panel's enablement —
is asked of the document, and the renderer's mask sampling reads the same
document mask that gates the brushes, so there is one source rather than two.

Old documents carry no mask and open unchanged; a mask painted and saved comes
back covering exactly what it covered.

## What the table must keep meaning

`ToolKind::verbs` is the one place that says where a tool applies. Every change
above is a row in it, and the shelf, the availability check, the diagnostics
report and the tests all read that row. The tests gain a table-driven pass that
asserts each declared pair actually reaches a distinct engine call, so a row
added without a binding fails rather than offering a tool that refuses.
