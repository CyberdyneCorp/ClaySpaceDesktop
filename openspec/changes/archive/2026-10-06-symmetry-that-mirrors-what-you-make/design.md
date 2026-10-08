# Design: an item carries its own mirror

## Context

The engine's mirror is a property of the layer (`clay_set_layer_mirror`), and
through ClayCore v0.120.1 an item could only take part in it or opt out. The
first two slices of this change (#277, #289) used the opt-out: an item made
with symmetry off stays out of any mirror the layer is given later, and a
real mirror change refills both images. What the opt-out could not express
was an item made under X when the layer's mirror was later turned off or
pointed at Z: the item still followed the layer, so its twin went or moved.

ClayCore v0.126.0 (#673, ABI 0.121.0) gives an item its own axes:
`clay_item_set_mirror_axes` on the builder and `clay_layer_set_node_mirror`
on a placed node, `CLAY_MIRROR_AXES_INHERIT` meaning "take the layer's". Own
axes replace the layer's for that item whatever its participation flag says;
the seam, the planes (the layer's local frame) and the radial mode stay the
layer's; a Move or a magnify reaches such an item through its own
reflections. Format minor 20 carries one byte per node for it.

## Decision

**Every item this application makes is written with the axes symmetry had
when it was made**, none included: stroke stamps, Puxar tendrils, curves,
placed objects, the armature (none — it mirrors itself) and a cut (none). The
`made_under` helper in `clayspace-engine` is the one place that says so. The
layer's mirror is no longer re-pointed by any of those verbs.

**The layer's mirror is still pointed by Move and Pinçar**, inside their own
gesture, at the symmetry the gesture is made with. Those two verbs are
reflected by the engine into every image of the items under them, and for an
item that *inherits* the layer's mirror the images are the layer's. Two kinds
of item inherit:

- the starting form, and the base shape of a subtool inserted as a shape
  (`insert_shape_subtool`), both left at `MirrorAxes::Inherit` on purpose;
- every item of a document saved before this change.

The starting form inherits because it is the body a sculptor drags with
symmetry off to pull one side out: an item holding X moves on both sides
under any drag, by the engine's rule, and a base sphere that did that would
make the simplest asymmetric gesture impossible. Standing on the mirror
planes, the form's reflection is itself, so for the field the choice is free;
it matters only to the drag images, which is exactly where "follow the
symmetry the drag is made with" is the right answer.

**The consequence, stated:** a Move or a Pinçar made with symmetry off moves
one side of the starting form and of items made with symmetry off, and moves
both images of an item made under symmetry, since both images *are* that
item. `a_drag_with_symmetry_off_on_an_item_made_under_symmetry_moves_both_images`
pins it. A host that wants a mirrored item to stop following its twin would
set that item's axes to none (`Document::set_node_mirror`), which the
application does not offer yet.

## Alternatives considered

- *Keep every verb pointing the layer's mirror, and add own axes on top.*
  Correct for the field, since own-axes items ignore the layer's mirror, but
  it keeps one engine edit per symmetry change inside every stroke's gesture,
  keeps re-pointing the layer retroactively for documents saved before this
  change on every stroke rather than only on a drag, and keeps the two-way
  accounting of opening entries for the live smooth and flatten. Nothing it
  buys is needed.
- *Give the starting form own axes too, so nothing inherits.* A drag with
  symmetry off on the base sphere would then move both sides, which the spec
  forbids and a sculptor would feel at once.
- *Bake a reflection on the host when the mirror changes.* Not expressible:
  a stamp, a curve or a placed object has no host-side reflected copy to
  make, and the engine has the axes now.

## What still reaches the layer's mirror, and how it is kept correct

- **Dirtying on a mirror change** (`point_the_mirror_of` →
  `mark_mirror_change`): unchanged. The engine's influence bound reads an
  item's own axes, so a node that carries its own has the same bound before
  and after the layer's mirror moves and is skipped; only inheriting nodes
  whose bound changed are marked, under both mirrors, and drained once. The
  tests drive the change through a Move on the starting form stood off the
  plane, the one inheriting item a test can make through the public API.
- **Undo**: a Move or a Pinçar that changes the layer's mirror still records
  that edit inside its gesture, and `forget_the_mirrors` still forgets the
  record after a history step so the next gesture reads it back. Strokes,
  curves, placed objects and bakes record nothing for the mirror now, and
  the live smooth and flatten owe no opening entries.
- **Magnify dirtying**: the engine reaches an own-axes item through its own
  reflections, so a magnify made with symmetry off can move the far side of
  an item that kept X. `axes_a_warp_can_reach` dirties the ball under the
  union of the layer's mirror and every axis an item on the layer carries,
  read through `clay_layer_node_mirror`. The union is over the whole layer
  rather than the items reached, which the magnify entry point does not
  report; a reflection nobody made costs a refill of empty bricks.
- **Move dirtying**: `clay_layer_move_surface_regions` already covers the
  twin of an item whose axes the layer's mirror does not set.
- **Tendril dirtying**: the tendril's own axes are the stroke's symmetry, so
  the tail regions are reflected through those directly.
- **Object drag preview**: reads `clay_layer_node_mirror`'s effective axes for
  the node, which is what the engine will draw, instead of the object table's
  flag crossed with the layer row.
- **Seam and planes**: the seam (`mirror_k`) is always 0 here, and the planes
  are the layer's local frame whichever axes are on, so neither needs the
  layer's axes set for an own-axes item to be reflected. Pinned by
  `an_item_made_under_symmetry_is_mirrored_on_a_layer_whose_mirror_is_off`.
- **Radial symmetry**: not used by this application.

## Documents saved before this change

Their items inherit, so they follow the layer's mirror as they always did: a
Move or a Pinçar made with another symmetry re-points it for all of them. New
items made in such a document carry their own axes. A document holding an
own-axes item cannot be written below format minor 20, which this application
never does.
