# The inspector answers what is being sculpted, under its own heading

## Why

Two sections of the right region were both headed `GEOMETRIA`. The polygon
counts stand under it, and so did the voxel display controls — and section
folds are keyed by the heading's word, so on a grid layer putting the counts
away also put the display controls away, and asking the interface where
`GEOMETRIA` was got whichever had been drawn last. One word, two sections, one
fold between them.

Underneath that is the thing the guide asks for and the panel did not have: a
place that answers *what can I control about what I am sculpting right now*. A
grid had display controls; a field and a mesh had nothing at all, and the
representation-specific parts that do exist are scattered — the combine
vocabulary in the options bar, the recorded passes under the layer stack, the
collapse advisory under the layer list.

## What Changes

- **One contextual section, in a fixed slot**, whose contents change with the
  active layer and whose position does not. A sculptor moving from a field to a
  grid finds the material and the geometry where they left them.
- **Each representation gets a heading of its own** — `CAMPO`, `VOXELS`,
  `MALHA` — which is what ends the fold collision. The voxel display controls
  move under `VOXELS`; the polygon counts keep `GEOMETRIA` to themselves.
- **A field states its edit list**: how many items it holds, and whether it has
  been collapsed. Drawn only where the engine has reported — a heading standing
  over nothing is a question left unanswered, and its height is not free.
- **A mesh states its topology contract**: the brushes move the vertices that
  are there and neither add nor remove any. Not a setting; the reason its
  brushes differ from a field's, and a sculptor who does not know it reads that
  difference as a bug.
- **An `inspector/` module** — `mod`, `sdf`, `voxel`, `mesh` — so the obvious
  home exists when a representation gains a control.

## What is deliberately absent, and why

The concept this is drawn from lists, per representation: field quality,
evaluation resolution, surface offset, field smoothness, voxel size, grid
resolution and bounds, filtering, box view, memory estimate, normals, and a
subdivision level.

**Voxel size and occupancy turned out to be real** — `clay_voxel_size` and
`clay_voxel_occupied_count` are bound in `claycore` and were being read inside
the engine adapter, so the interface could say a layer held voxels and not how
coarse they were. They are in the grid's section now; see
`a-stretch-the-engine-already-had` for the audit that found them and the
per-axis scale in the same sweep.

**Of the rest, not one is a value this application's domain or the pinned
engine can express per layer.** Drawing a control for something
nothing reads is an interface that lies about what the program does, so none of
them is here. The guide that asks for this panel says the same thing in its own
words, about a resolution setting it declined to invent.

Three things that *do* exist stay where they are rather than being duplicated
into this section:

- the **combine** operation, its join profile and its radius belong to the
  stroke, not the layer, and stand in the options bar with the stroke's other
  numbers;
- a grid's **recorded passes** are nested under the layer they were recorded
  on, because a pass has no meaning apart from that grid, and the control that
  starts one belongs beside them;
- the offer to **collapse** a costly field appears under the layer list, and
  only while the engine is advising it — a row that is always there is a row
  nobody reads.

Each would otherwise give a sculptor two places to look and two places to keep
in agreement.
