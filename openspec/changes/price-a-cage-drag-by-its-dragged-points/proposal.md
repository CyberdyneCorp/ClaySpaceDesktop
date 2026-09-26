# Price a cage drag by the points that were dragged

Issue #176. Dragging one control point of a 32³ cage on a mesh subtool cost
4.7–5.2 s a frame on the 80k-triangle fixture (1.7 s on the 62k-vertex
starting mesh here), against ~10 ms at 3³. The cost tracked the cage's size,
not what was in hand.

## Why it happened

The mesh preview lays the cage over the mesh on every pointer move, and the
engine's evaluation of a cage — `clay_mesh_lattice_displacement`, applied once
per vertex — summed every control point of the cage whether or not it had been
dragged: 32,768 terms a vertex at 32³ for one corner. A point at rest adds
exactly nothing, so nearly all of that was work with no result. Nothing on this
side was proportional to the cage; the engine's sum was. There was also no
figure anywhere that said which cage a slow frame belonged to or how long it
took.

The same audit reported the cage sized from stale bounds after a move (±0.96
around a ball of radius 0.5). Measured, the bounds are current: a new subtool
is mirrored on X, so a ball moved off the axis leaves a mirrored copy and the
cage correctly encloses both. With the mirror off the cage is the moved ball's
own box.

## What changes

- **Engine** (ClayCore#655): a mesh cage keeps the set of dragged control
  points and sums over that set alone, with an O(n) basis per axis. Measured
  here with that engine: 32³ single-point frame ~1.7 s → ~11 ms, 3³ and 8³
  ~9–10 ms. Reaches this application when the pin moves to a release carrying
  it.
- **Reported**: `LatticeState` carries `dragged` (points away from rest) and
  `preview_micros` (the last mesh preview frame); the agent's cage state reads
  them as `dragged_points` and `preview_ms`.
- **Pinned down**: preview and apply agree bit for bit (positions and normals);
  the cage is sized from the form's current bounds, mirrored copies included;
  and a ClayCore repro records the engine's price of an evaluation against the
  cage's size, which flips when the pin moves.

## Out of scope

The footprint growth during a drag is the per-frame GPU buffer churn tracked by
the GPU-memory issue; the document's own accounted memory does not move across
a drag.
