# Release a cage drag's upload staging

Issue #176, the memory half. The issue reported the footprint going from
1.30 GB to 3.5 GB across 32³ cage drags and staying there. `measure-a-cage-drag`
added `cage.memory`, which read **1.00×**, and the engine test showed the
document's accounted memory flat across a drag, so the growth was put down to
GPU buffer churn already fixed by #166. It was not.

## Why

Measured on current main, on the 296k-triangle mesh reference with one corner
of an 8³ cage dragged for 100 frames through the harness's viewport path:

| point | process footprint | device gauge |
|---|---|---|
| before the drag | 133 MB | 36.1 MB |
| frame 20 | 368 MB | 36.1 MB |
| frame 60 | 752 MB | 36.1 MB |
| frame 100 | 1,132 MB | 36.1 MB |
| cage cancelled | 1,151 MB | 36.1 MB |

About 9.5 MB a frame, which is one upload of the carried mesh (vertices and
indices), kept after the cage came down. The same drag without the viewport
upload stayed at 132–197 MB, so the engine is not where it goes. The same drag
with an empty queue submission after each frame stayed at 250–309 MB.

The cause is wgpu's pending writes. A `write_buffer` takes a staging copy that
wgpu keeps until a submission carries it, and frees only after the device has
finished with that submission. The buffers themselves are reused (#166), which
is what the device gauge counts, but the staging of every upload whose frame
never submits stays. The application's frame uploads the carried mesh before
it acquires the window's image, and returns without submitting when that
acquire times out or needs a reconfigure; the benchmark harness has no window
and never submits at all.

The device gauge missed it because `note_device_idle` cleared all staging,
submitted or not. A wait releases only what a submission carried.

## What changes

- **`Gpu::flush_writes`**: hands every write so far to the device in an empty
  submission. `Renderer::set_mesh_layers` ends with it, so a whole-mesh upload
  (every cage drag frame) never waits for a frame to release its staging.
- **A skipped frame flushes.** When the window's image cannot be acquired, the
  frame's uploads are flushed before it returns instead of waiting for the next
  frame that manages to submit.
- **The device ledger counts unsubmitted staging as held.** Staging is open
  (not submitted), flushed (submitted outside a frame) or submitted (the last
  frame's). A wait releases the last two only. `Gpu::note_submitted` marks a
  caller's own submission; the offscreen capture uses it.
- **`cage.footprint`** in the benchmark: the process footprint after a 100-frame
  drag, released, over before it, budget 1.2×. It checks the same thing as
  `cage.memory` without relying on the gauge.

Measured after the change (M3 Pro, engine v0.120.1, loaded machine):

| figure | before the fix (ledger corrected) | after |
|---|---|---|
| `cage.memory` | 5.66× | 1.00× |
| `cage.footprint` | 2.63× | 1.03× |

## Out of scope

The per-frame cost of a large cage (`cage.drag_32.ms` is seconds). That comes
from the engine's evaluation and waits on a ClayCore release carrying #655; see
`price-a-cage-drag-by-its-dragged-points`.
