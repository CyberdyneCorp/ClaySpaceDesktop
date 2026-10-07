# Proposal

## Why

A live drag of an object placed with **Interseção** refilled the whole layer on
every frame. On the `macos-14` runner it cost 2.73x the same drag with
**Subtração** (median of twelve runs), and on `reference-10x` it took about a
second per frame (#282). An intersect's influence bound really is its layer,
because `max(acc, item)` removes material anywhere the layer has any. But a
move only changes the surface inside the sweep of where the operand was and
where it went. ClayCore has reported that narrower region since ABI 0.90.0
through `clay_layer_set_transform_bound` (ClayCore#471), and the application
never called it.

While verifying the narrow refill against the document, placing an intersect
operand turned out to leave the whole un-intersected form in the brick cache.
The item was added at the origin and then moved into place, two engine edits
with no refill between them, and ClayCore v0.120.1 refills the bricks outside
the move's sweep from seeds that predate the add (ClayCore#665).

## What Changes

- An object move with a uniform scale goes through
  `clay_layer_set_transform_bound`. The refill is the overlap of the region it
  reports with the node's influence bound before and after. Each is a sound
  bound on what the move changed, and on a worked form each is loose in a
  different direction: the engine's region is the sweep dilated by the chain
  pad and can reach past the layer, and the influence bound is the layer.
- A per-axis scale keeps the per-axis setter and the influence bounds. The
  engine has no narrow answer for a squashed operand, and the uniform setter
  would collapse the stretch.
- A placed shape is built at its position and added in one edit, so it does
  not go through the add-then-move sequence behind ClayCore#665.

What was left of #282 at engine 0.120.1 was on the engine side: the region
was the sweep dilated by the whole layer's chain pad (about 0.47 at the
reference size, 1.48 at ten times it), which kept an intersect frame at 3,360
and about 25,000 brick keys where the subtracting control was about 1,000
(ClayCore#666). ClayCore v0.126.0 pads the region by the sum of the supports
of the combines after the operand (ClayCore#676). A placed object is appended
last and carries no pad, so an intersect frame now refills the bricks its
subtracting control does on both scenes, and the application takes that up
without a code change. The scaling test is tightened to that answer so a
return of the layer-wide pad fails it, and the documentation carries the
measured figures.

## Capabilities

### Modified Capabilities

- `object-transform`: a live operand's drag refills what the move changed, and
  the cached surface agrees with the document for an intersect.

## Impact

`claycore` gains a binding for `clay_layer_set_transform_bound`. In
`clayspace-engine`, the object transform and placement paths change, with an
`objects::clip` helper and a new `tests/intersect_drag.rs`. `clayspace-app`
gains `tests/intersect_drag_scaling.rs`. The docs change in `docs/features.md`
and `benchmarks/ci-gate.md`.
