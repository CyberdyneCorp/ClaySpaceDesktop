# Move the engine pin to ClayCore v0.52.2

## Why

The pinned engine is v0.39.0. v0.52.2 is thirteen minor versions ahead and its
release notes claim large gains on paths this application spends its time in.
Measured against our own benchmark suite, most of them hold: the whole suite
runs in 156 s against 309 s, flatten and relax are about 20x faster, mesh
export 10.9x. Two things do not, and both are the reason this is a change
rather than a one-line pin bump.

**One figure regressed.** `brush.sdf.mover` costs 1.76x what it did. Localised
to `clay_brick_cache_refill` re-meshing an identical set of bricks — same warp
nodes, same brick counts, same boxes — at 1.82x the cost. Reported upstream as
CyberdyneCorp/ClayCore#335. We take the regression knowingly: it is one brush
against a suite that got twice as fast overall.

**Undo across a crossing changed under us.** `unify-the-undo-history` made the
filling of a converted layer undoable without making layer creation undoable,
so an engine undo now empties the new layer and leaves it standing. Measured
across the pin, one undo of the same crossing: v0.39.0 left the layer's 3,952
vertices alone; v0.52.2 left the layer in the list at zero. A sculptor who
crosses a representation and presses undo watches their model vanish and is
left with a layer nobody asked for.

That is a behaviour change the specification has to answer, because the
specification currently says a conversion is not undoable at all and explains
why — an explanation that stopped being true.

## What changes

- The engine pin and `EXPECTED_ABI` move to v0.52.2.
- A crossing becomes one undo step: undo takes the layer off the scene as well
  as taking back its filling, and redo puts both back. This is the reverse of
  what the specification said, and it is now the better answer available:
  the engine records the filling, so the host only has to carry the layer.
- The Linux baseline is re-recorded, since brick counts and mesh output move.

## Impact

- `representation-conversion`: the requirement that a conversion is not
  undoable is replaced by one that says it is, in one step.
- `performance-budgets`: the recorded baseline moves. The mover figure moves
  the wrong way and is recorded as such rather than hidden.
- Hosts reading a saved file see no change: an undone crossing's layer is
  dropped at save, since a file has no redo stack to restore it from.
