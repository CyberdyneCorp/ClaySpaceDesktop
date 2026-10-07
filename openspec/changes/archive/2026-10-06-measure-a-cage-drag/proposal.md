# Measure a cage drag, frame by frame

Issue #176, the measurement half. `price-a-cage-drag-by-its-dragged-points`
states the budget a mesh cage drag has to hold and pins down what the preview
guarantees. The engine side (ClayCore#655) is written but not yet released, so
the pin cannot move. What was missing on this side is the instrument that
says, on every run, whether the budget holds, so that the pin move is a
measured change rather than a described one.

## Why

The only cage figure the benchmark carried was `op.mesh.lattice_drag`, a cage
laid down once as an operation. The cost a sculptor feels is different: a drag
on one control point of a raised cage, repeated on every pointer move, and
whether that cost follows the points in hand or the size of the cage. Nothing
recorded it. Nothing recorded whether a long drag leaves memory behind either,
which is the second half of the issue's report (1.30 GB to 3.5 GB, not
released).

## What changes

- **`cage` benchmark group** over the mesh reference (296k triangles):
  - `cage.drag_3.ms`, `cage.drag_8.ms`, `cage.drag_32.ms`: one corner of a
    3³, 8³ and 32³ cage dragged, timed from the pointer move to the surface
    uploaded, with the 16 ms interface-thread budget attached to each.
  - `cage.scaling`: the 32³ frame over the 3³ frame, budget 3×. The
    machine-independent form of "priced by the points dragged".
  - `cage.memory`: device memory after a 100-frame drag, released, over
    device memory before it, budget 1.2×.

  The budgets are reported and not enforced, like every other budget in the
  harness. They become enforceable when the pin carries ClayCore#655.
- **Engine test**: a 100-frame drag does not grow the document's accounted
  memory with the frame count, and taking the cage down returns it to within
  20% of where it started.

Measured on a workstation (Apple M3 Pro, engine v0.120.1, loaded machine):

| figure | value |
|---|---|
| `cage.drag_3.ms` | 45.9 ms |
| `cage.drag_8.ms` | 85.7 ms |
| `cage.drag_32.ms` | 3,924 ms |
| `cage.scaling` | 85.6× |
| `cage.memory` | 1.00× |

The frame figures reproduce the issue's (57 ms, 150–188 ms, 4.7–5.2 s). The
memory figure says the per-frame upload already goes into the buffers the
surface holds (#166), so a drag leaves nothing on the device.

## Out of scope

Moving the engine pin, flipping
`a_mesh_cage_evaluation_is_priced_by_every_point_the_cage_holds` and enforcing
the frame budget. All three wait on a ClayCore release carrying #655 and are
tracked in `price-a-cage-drag-by-its-dragged-points`.
