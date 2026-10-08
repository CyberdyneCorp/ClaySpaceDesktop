# Symmetry that mirrors what you make

## Why

Symmetry is stored as a property of the *layer*: `clay_set_layer_mirror`
reflects every item the layer holds, past and future. Every item took part by
default, so the switch reached backwards (#170):

- **Retroactive mirroring (Y2).** A lump sculpted with symmetry off grew a twin
  as soon as a later stroke turned the layer's mirror on; a box placed
  one-sided became two.
- **Ghost surfaces (Y1).** A mirror change moves surface the brick cache holds,
  and only the stroke's own region was refilled. The reflections an old mirror
  made stayed drawn — on hidden layers too, because hiding refills only what the
  layer reaches now.
- **Curves ignored it (C9).** A curve never pointed the layer's mirror, so one
  begun with symmetry on stayed one-sided until a brush stroke happened to
  write it.

## What changes

- **An item carries the axes it was made under.** Every item this
  application makes — stroke stamps, snakehook tendrils, curves, placed
  objects — is given the mirror axes symmetry had when it was made
  (`clay_item_set_mirror_axes`, ClayCore v0.126.0), none when it was off.
  The engine reflects the item through those whatever the layer's mirror is
  pointed at, so turning symmetry on, off or to another axis leaves what is
  already there as it was, through a save and a reopen as well. The armature
  and a cut carry none: a rig mirrors itself, and a cut is drawn in the
  sculptor's own sight.
- **The layer's mirror is written by Move and Pinçar only**, inside their
  own gesture, because the engine reflects a drag or a magnify into every
  image of the items under it and the images of an item that *inherits* the
  layer's mirror — the starting form, the base shape of an inserted subtool,
  and every item of a document saved before this change — are the layer's.
  A stamping stroke, a pull, a curve, a placement and the bake verbs no
  longer touch it, so a symmetry change costs them no engine edit.
- **A drag on an item made under symmetry moves both images**, whatever
  symmetry the drag is made with: both images are that item, by the engine's
  rule. The starting form inherits the layer's mirror so that a drag with
  symmetry off still moves one side of it.
- A real change of the layer's mirror marks the layer under the old mirror
  and under the new one, and drains once; an item carrying its own axes has
  the same bound either way and costs nothing on the change. A magnify
  dirties under the union of the layer's mirror and the axes items on the
  layer carry.
- A curve and a placed object made with symmetry on are mirrored from the
  moment they are placed, in one undo step.
- **A Move on the mirror plane is applied once** (F10): ClayCore v0.126.0
  resolves a drag's coincident images as one grab, measured at +0.1458
  mirrored against +0.1458 unmirrored where v0.120.1 gave 0.2307 (1.58x).
- **Rigs (A5).** An armature mirrors itself, so its item carries no mirror
  axes: a stroke made with symmetry on on a rig's subtool no longer gives a
  sphere added one-sided a twin. And a rig edit, which removes the armature
  and places it again, puts it back where it stood in the layer's order
  (`clay_layer_move`) instead of appending it, where it filled in every carve
  made into the rig since.
- The `toggle_symmetry` action and the tool state's `symmetry` on the agent
  surface describe the axes as what the brush makes *next*.

## Impact

Documents saved before this change hold items that inherit their layer's
mirror, so a Move or a Pinçar made with another symmetry still re-points that
mirror for all of them, as it always did; items made from now on carry their
own axes. A document holding an own-axes item cannot be written below format
minor 20, which this application never does.

The `claycore` crate gains `MirrorAxes`, `Item::set_mirror_axes` /
`mirror_axes`, `Document::set_node_mirror` / `node_mirror` and
`Document::add_group`.
