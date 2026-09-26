# Proposal

## Why

Two field tools break the constraints the rest of the toolset keeps (#177).
Snake Hook sampled the mask only at the path's own samples, so the tube
joining the samples either side of a masked band ran straight through it and
the band was pulled as much as open surface (0.270 masked against 0.269
open). The same path let the root swell as the pull extended, let the tip run
a whole tip radius past the pointer, and met the surface at a hard crease the
grid samples into a sawtooth. Move Topológico folded on a long drag — the
engine re-samples through the inverse of the move, which stops being one to
one once the displacement outruns the falloff — and, delivered in segments on
a field, each segment was anchored at the pointer rather than at the material
the last one carried, ending in a shelf with a cliff. It also ignored the
mask.

## What Changes

- Snake Hook gates its curve item by the layer's mask, over everything the
  tendril reaches, as the stamp verbs do.
- The chain's links are no longer smooth-unioned into one another; the
  tendril joins the surface through a fillet of half the brush instead.
- The curve is trimmed by its tip radius so the rounded cap ends at the
  pointer, and is tessellated at the document tolerance from the first
  segment so a grown pull never re-tessellates spans already placed.
- Move Topológico is applied in steps of a quarter of its reach, each anchored
  where the last carried the material, with a crossfade band covering the
  whole drag, and is gated by the mask.
- On a field, Move Topológico holds the whole gesture and lands once when the
  pointer comes up.

## Capabilities

### Modified Capabilities

- `sculpting-tools`: Snake Hook honours the mask and ends where its path
  ends; Move Topológico lands whole without tearing.

## Impact

`clayspace-engine` (`snakehook_stroke`, `topological_move_stroke`),
`clayspace-model` (`ToolKind::holds_the_whole_gesture`), a `claycore` binding
for the curve tolerance, and regressions in `clayspace-engine`.
