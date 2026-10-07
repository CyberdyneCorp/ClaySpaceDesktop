# Hold the first drag where it is measured

Issue #196, the two rows left after #284, #292 and #296: **D14**, the first
gizmo drag after `set_mode` stalling about 1.1 s in the audit, and **I14**,
`set_grid_display` stalling with no grid in view. And the shadow both cast on
CI: main's macOS jobs have been red since the two first-drag budgets landed
(#312, #314), in debug and in release, on the budgets and not on the drag.

## Why

**D14, profiled rather than guessed.** The first drag frame of the mesh
reference was split by phase on an Apple M3 Pro (Metal, a shared machine) and
compared with the second and third frame of the same drag, through the path
`gizmo_first_drag.rs` times — the document edit, `SurfaceGeometry::sync` and
the carried rebuild and upload — with the application's mask sampling beside
it:

| phase, per drag frame | release, frames 1–3 | debug, frames 1–3 |
|---|---|---|
| document edit | 0.24–0.28 ms | 0.26–0.36 ms |
| surface sync | 0 (no brick) | 0 |
| read the mesh layer from the engine | 0.20–0.24 ms | 0.22 ms |
| place its 148,122 vertices and normals | 0.53 ms | 139–212 ms |
| append to the carried buffer | 0.22–0.46 ms | 29–52 ms |
| mask sampling (application only) | 0.05 ms | 43–130 ms |
| vertex zip | 0.75 ms | 31–37 ms |
| GPU upload | 1.1–1.4 ms | 4–12 ms |
| span bounds | 0.55 ms | 45–70 ms |
| flush | 0.04–1.5 ms | 0.2–3.9 ms |
| **frame** | **3.9–5.5 ms** | **373–431 ms** |

The first frame costs what every later one costs. Nothing is built lazily on
it. The one-time costs — the GPU buffers' allocation (2.6 ms) and the first
touch of a fresh vertex buffer (16.6 ms) — belong to the carried build at scene
open, which precedes any drag and is outside the timed window. A placed
object's press is the same shape: meshing the primitive alone 3.4–5.9 ms,
posing 0.6 ms, upload 0.9 ms in release, and the second and third press read
the same as the first. Debug is the same work in the same per-vertex loops,
sixty to eighty times slower, which is the profile and not the drag.

Through the running application (the door, release, Metal), the first
`transform/drag` after `transform/set_mode` on an SDF layer, on a mesh crossed
from the field and on a placed object read 5.6–10.5 ms round trip, the same as
the second drag's; the release of an SDF drag is the 50–62 ms field write it
has always been. The audit's 1.1 s does not reproduce on this build. The
refill it most plausibly was — 45–98 ms on the first object frame, 50–57 ms on
every mesh frame in #312's figures — is what #300, #312 and #314 removed.

**The gate.** Both millisecond budgets in `gizmo_first_drag.rs` were red on
every macOS job from the day they landed: 95–142 ms in debug and 17.4, 32.6
and 46.0 ms in release on `macos-14`, against 16.7 ms, for a frame this Mac
reads at 4.5 ms. `benchmarks/ci-gate.md` already states the position — a
hosted Mac is a three-core virtual machine at a load of four to eight per
core, single figures move by up to 10x between runs, and a gate that fails on
the runner's noise is one people learn to ignore — and `sculpt_latency`,
`gesture_end` and `visual_brushes` already decline to assert a timing in a
debug build. The budgets are now held where they mean something and printed
elsewhere, with the reason, and the counts beside them hold everywhere.

**The undo frame.** `an_undo_draws_the_frame_it_took_back` fails on the debug
jobs with 108 pixels at up to 3 levels. The artifacts show it is the debug
build's *first* frame that is off: the undone frame matches the release jobs'
frames exactly, and the same 108 pixels at 1–3 levels separate the debug and
release jobs' first capture of the starting form in four unrelated tests of
the same run, on both backends, on three runs. The undo is exact; the debug
build's first picture is a one-level speckle away from release's. The exact
allowance takes the measured count in a debug build or on a hosted runner and
keeps its handful on a workstation in release.

**I14.** Two attempts on main could not reproduce it (0.034–0.050 ms per call,
no upload), `agent_end_to_end.rs` holds the audit's shape through the window
with six sculpted grids hidden, and `docs/features.md` records the audit's
figure as the hidden-grid smoothing #239 removed. The row is closed on that
evidence.

## What changes

- `support::hold_to_budget` and `support::budget_verdict` in the
  application's test support: a budget is a verdict in release off a hosted
  runner (`CI` unset) and a printed figure elsewhere. The rule is a pure
  function with its own test.
- The two budgets in `gizmo_first_drag.rs` go through it. The release budget
  is unchanged, and so are the structural assertions.
- `visual_incremental.rs`'s exact allowance is 128 pixels in a debug build or
  on a hosted runner (the CPU-only release job read the same 108 on 7 October),
  16 in release, at the same levels.
- `performance-budgets`: a millisecond budget is asserted only in an optimised
  build off a hosted runner; elsewhere the figure is printed and the
  machine-independent properties are asserted.
- `docs/features.md` and `docs/roadmap.md` record the profile, the gate rule
  and the closing of I14.

## Out of scope

Making the carried rebuild independent of the mesh's size — drawing a carried
subtool's drag through a per-span transform on the GPU, as #300 draws an SDF
layer's — would take the drag frame from proportional to the mesh to a
constant, and is the follow-up if a larger mesh needs it. On the reference
mesh the frame is a quarter of the budget. `a_long_pull_keeps_its_segment_cost`
is a late-over-early ratio on engine work compiled the same in both profiles,
not a millisecond budget, and is left as it is; it failed once in three runs,
at 6.5 against 6.0, in a debug job.
