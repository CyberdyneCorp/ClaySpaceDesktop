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

- An item decides at creation whether it takes part in its layer's mirror
  (`clay_item_set_mirror`): made with symmetry on it follows the mirror, made
  with it off it stays out of every mirror the layer is given later. Stroke
  stamps, snakehook tendrils, curves and placed objects all carry the flag.
- A real mirror change marks the layer under the old mirror and under the new
  one, and drains once, so neither image is left stale.
- A curve and a placed object made with symmetry on point the layer's mirror in
  the same undo group as the item, so they are mirrored from the start and one
  undo takes back both. Made with symmetry off they leave the layer's mirror
  alone: nothing they do needs it.

## Not in this change

The per-layer engine mirror stays. What remains of #170 needs either the
engine's per-item axes or a host-side bake, and is tracked on the issue:

- Turning symmetry **off**, or switching it to a different axis, still re-points
  the layer's mirror and so still changes items made under the old one.
- A Move drag exactly on the mirror plane is applied once per image (F10); the
  drag images are the engine's.
- Rig edits and the rig layer's strokes (A5).

## Impact

Documents saved before this change keep each item's participation as it was
saved. Only items made from now on carry the new decision.
