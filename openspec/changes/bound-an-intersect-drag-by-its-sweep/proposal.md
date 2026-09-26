# Proposal

## Why

A live drag of an object placed with **Interseção** refilled the whole layer on
every frame. On the `macos-14` runner it cost 2.73x the same drag with
**Subtração** (median of twelve runs), and on `reference-10x` it took seconds
per frame (#282). An intersect's influence bound really is its layer, because
`max(acc, item)` removes material anywhere the layer has any. But a move only
changes the surface inside the swept union of where the operand was and where
it went. ClayCore has reported that narrower region since ABI 0.90.0 through
`clay_layer_set_transform_bound` (ClayCore#471), and the application never
called it.

While verifying the narrow refill against the document, placing an intersect
operand turned out to leave the whole un-intersected form in the brick cache.
The item was added at the origin and then moved into place, two engine edits
with no refill between them, and ClayCore v0.120.1 refills the bricks outside
the move's sweep from seeds that predate the add (ClayCore#665).

## What Changes

- An object move with a uniform scale goes through
  `clay_layer_set_transform_bound` and refills the region the engine reports.
  For an intersect operand that region is the sweep; for every other operation
  it is the same before/after union as before.
- A per-axis scale keeps the per-axis setter and the influence bounds on both
  sides. The engine has no fast path for a squashed operand, and the uniform
  setter would collapse the stretch.
- A placed shape is built at its position and added in one edit, so it does
  not go through the add-then-move sequence behind ClayCore#665.

## Capabilities

### Modified Capabilities

- `object-transform`: a live operand's drag refills what the move changed, and
  the cached surface agrees with the document for an intersect.

## Impact

`claycore` gains a binding for `clay_layer_set_transform_bound`. In
`clayspace-engine`, the object transform and placement paths change, with a new
`tests/intersect_drag.rs`. The docs change in `docs/features.md` and
`benchmarks/ci-gate.md`.
