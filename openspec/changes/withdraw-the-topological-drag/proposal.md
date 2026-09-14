# Withdraw Mover Topológico

## Why

Issue #128. Move Topológico on an SDF layer did not deform the surface: it
replaced a square patch of it with the lattice the bake was sampled on — hard
stair-stepping against the untouched field, reported from ordinary use before
any probe was run.

**The cause is the verb, not how we call it.** Measured on a clean sphere with
the reported stroke — brush 0.35, intensity 1.0, a three-sample 0.4 pull off
the pole — through the tool's own bake-move-replace path:

| what was varied | result |
|---|---|
| the round trip alone: bake the region, replace it, move nothing | roughness **0.015**, which is the untouched sphere's own |
| feather 0, 0.02, 0.06, 0.12, 0.24 | identical to four decimals |
| band 0.06 (the default), 0.2, 0.6, 1.2 | identical to three decimals |
| cell 0.02 → 0.01 → 0.005 | **worse**: normal swing 3.60° → 5.84° → 7.74° |
| the drag split into 2, 4, 8, 16 legs | better, never clean; the lattice is in every frame |
| relaxing the moved volume afterwards | costs displacement, leaves a hard square frame of lattice |

So the bake is innocent — it puts a region back untouched to four decimals —
and nothing the caller passes changes the result. What does change it is the
drag: the relief grows with the displacement and falls with the reach, and
`relief × reach / displacement` holds to ±3% across an eightfold change of
reach. That expression is the weight the verb applies — `w = 1 − g/radius`, so
a surface displaced by `w·d` carries any error in `g` scaled by `d/radius` —
and it prices the error at about 3.5 × 10⁻³, a sixth of a cell, *independent of
the reach*.

An error that is a fixed fraction of a cell however long the path is is a
quantisation rather than a drift, and the engine's source says which one: `g`
is a Dijkstra geodesic over a 26-neighbour lattice, so every value it can hold
is a sum of `cell`, `√2·cell` and `√3·cell` steps, locked to the grid and
sampled back out trilinearly. The weight ripples at the cell wavelength, the
drag multiplies that ripple onto the surface — 4 × 10⁻³ of relief over a 0.02
wavelength on the reported stroke, a slope of about 0.4 — and a finer cell
shrinks the wavelength rather than the error, which is why halving it makes the
result worse.

That is inside `clay_item_volume_move_topological`, on the far side of the C
ABI. The tool has no binding on any other representation, so with the SDF verb
withdrawn nothing is left of it.

## What Changes

- **`ToolKind::MoverTopologico` is removed** — from `ALL`, from the verb table,
  from the key and label, from the glyph set, from all three languages' strings,
  and from the engine's stroke routing along with `topological_move_stroke`.
- **The outcome is pinned in a frame, not in a table.** A new visual test
  strokes every tool the shelf offers on a field with the reported gesture and
  measures the rendered surface against an untouched sphere in the same run.
  Every brush that shapes the surface lands between 1.01x and 1.31x; the
  withdrawn one was at 2.07x.

## Out of scope, and why

- **No ClayCore change.** The defect is in the engine's geodesic solve, and
  repairing it is the engine team's call, not a workaround here.
- **The reach was real and is not disputed.** A horseshoe fixture measured the
  drag declining to cross a gap — +0.295 near tip against +0.000 far, where the
  Euclidean drag carried +0.167 and +0.089. It went out with the tool because
  there is no tool left to measure; that measurement is the case for asking the
  engine to fix the weight, not for keeping a brush that damages the surface.
