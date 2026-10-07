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
  points and sums over that set alone, with an O(n) basis per axis. Pinned
  from ClayCore v0.126.0. Measured on that pin (Apple M3 Pro, Metal, machine
  under load), one corner dragged:

  | figure | v0.120.1 | v0.126.0 | line |
  |---|---|---|---|
  | engine frame, 62,576-vertex starting mesh, 3³ (best of 6) | ~10 ms | 7.5 ms | — |
  | engine frame, same mesh, 8³ | ~35 ms | 7.7 ms | — |
  | engine frame, same mesh, 32³ | ~1.7 s | 9.9 ms | 16 ms × 10 |
  | `cage.drag_3.ms`, 296k-triangle reference, to the surface arriving | 45.9 ms | 22.6 ms | 16 ms |
  | `cage.drag_8.ms` | 85.7 ms | 23.0 ms | 16 ms |
  | `cage.drag_32.ms` | 3.9 s | 27.1 ms | 16 ms |
  | `cage.scaling` (32³ over 3³) | 85.6× | 1.20× | 3× |
  | `cage.memory` | 1.00× | 1.00× | 1.2× |
  | `cage.footprint` | 1.03× | 1.00× | 1.2× |
  | ClayCore repro, 2,048 evaluations, 32³ over 3³ | 755× | 9× | 20× |

  The 32³ single-point frame holds the 16 ms budget on the issue's scale of
  mesh, and `one_corner_of_the_largest_cage_holds_a_frame` holds it there at
  the CI gate's own margin of ten. On the 296k-triangle reference every size
  reads over 16 ms by 18–20 ms of engine time spent bending the whole mesh
  and about 3.5 ms uploading it, the same at 3³ as at 32³; what the cage adds
  is the 4.5 ms between them. The benchmark budget stays attached to all three
  sizes and is reported like every other budget in the harness; the figures
  are absent from the committed baselines, so the gate reports them as `new`
  until the baselines are re-recorded.
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
