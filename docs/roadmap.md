# Roadmap

Where the project stands, what is left, and what is still undecided. Task
counts come from `openspec/changes/add-clayspace-desktop/tasks.md`, which is
the authority.

**94 of 109 tasks. Milestones 1 to 4 delivered; milestone 5 in progress.**

Engine pinned at ClayCore **0.27.3**. A dab is 15.8 ms median and 23.1 ms p95
on a bare document, against a 50/100 ms budget — but 81 ms on the reference
scene, for a reason that is upstream and understood; see *What is slow and
why*.

## Milestones

| | Milestone | State | What it means |
|---|---|---|---|
| M1 | Engine bridge | Delivered | Submodule, CMake build, generated FFI, safe wrapper, verified against every registered backend |
| M2 | Viewport | Delivered | Window, wgpu device, MatCap, camera, overlays, gizmo, device-loss recovery |
| M3 | Sculpt loop | Delivered | Live strokes, incremental re-mesh, brush cursor, undo as one action per gesture |
| M4 | Interface shell | Delivered | Panels, scene tree, layer stack, inspectors, design system |
| M5 | Vocabulary, I/O, packaging | **In progress** | Masks and armatures, documents, performance gates, bundles |

## Task groups

| Group | Milestone | Done |
|---|---|---|
| 1. Workspace and engine bridge | M1 | 16/16 |
| 2. Acceleration policy | M1 | 5/7 |
| 3. Rendering foundation | M2 | 8/9 |
| 4. MVVM skeleton | M3 | 7/7 |
| 5. Sculpting loop | M3 | 11/11 |
| 6. Masks and armatures | M5 | 6/6 |
| 7. Scene, layers and history | M4 | 13/13 |
| 8. Document lifecycle | M5 | **2/8** |
| 9. Interface shell and design system | M4 | 16/16 |
| 10. Performance and packaging | M5 | **9/13** |
| 11. Close-out | M5 | 1/3 |

## What is blocked, and what is not

**Nothing on the remaining task list is blocked by ClayCore.** Every open
upstream issue degrades something already built rather than stopping something
next. The one exception is CI, where two of seven jobs cannot pass until the
engine compiles on the CI compiler.

### Upstream: fixed on `main`, awaiting a release

These are merged in ClayCore and not yet tagged. Each one lets us delete a
workaround; the tests named will start failing when the fix arrives, which is
the signal to do it.

| Issue | What it unblocks here | Test that flips |
|---|---|---|
| [#60](https://github.com/CyberdyneCorp/ClayCore/issues/60) layer mirror had no effect | Turn the starting symmetry back on | `claycore_repros::the_layer_mirror_has_no_observable_effect` |
| [#61](https://github.com/CyberdyneCorp/ClayCore/issues/61) `CLAY_OP_ADD` ignored stroke strength | Intensidade starts working for the Add-based tools | `claycore_repros::op_add_ignores_the_stroke_presets_strength` |
| [#62](https://github.com/CyberdyneCorp/ClayCore/issues/62) relief amplitude undocumented | Nothing to undo — the mapping is already correct, this makes it checkable | — |
| [#66](https://github.com/CyberdyneCorp/ClayCore/issues/66) subset meshing omitted straddlers | Drop the whole-surface `settle` after every stroke, and the mid-drag seams with it | `claycore_repros::subset_meshing_reproduces_whole_surface_meshing` |

### Upstream: still open

| Issue | Effect here | Our position |
|---|---|---|
| [#71](https://github.com/CyberdyneCorp/ClayCore/issues/71) 0.27.3 will not build with AppleClang | **Both macOS CI rows fail** | The only true blocker. One character upstream; we are not patching a vendored engine |
| [#73](https://github.com/CyberdyneCorp/ClayCore/issues/73) gradient normals scale with document size | A dab is 4 ms on a fresh document and 120 ms after 192 edits | Workaround available: host-side normals. Not taken — see below |
| [#67](https://github.com/CyberdyneCorp/ClayCore/issues/67) bake-and-replace corrugates the surface | Suavizar, Relaxar, Planar and Polir damage what they touch | Applied once per gesture rather than per segment, which halves it. Still visibly wrong |
| [#69](https://github.com/CyberdyneCorp/ClayCore/issues/69) no layer enumeration | A reopened document loses layer names, visibility and **stack order** | Layer ids recovered by probing. Order loss is a silent correctness difference |
| no armature topology readback | A saved rig comes back as surface without its tree, so it cannot be re-posed | Verified in `armature_persistence.rs`. Not yet filed; it belongs beside #69 |
| [#64](https://github.com/CyberdyneCorp/ClayCore/issues/64) Metal 7–10× slower than CPU at refill | None — routed around | `BackendPolicy::refill_backend` returns CPU; `backend_choice.rs` fails when that flips |
| [#63](https://github.com/CyberdyneCorp/ClayCore/issues/63) Metal absent on paravirtual GPUs | None for us | Not ours; kernel half fixed in 0.27.3 |

## What we can do now

In the order I would take it. None of this waits on anyone.

**1. Autosave and recent files — 8.5, 8.7.** Pure host work, and autosave
matters more than usual while four tools can damage a surface.

**2. Mesh import and export — 8.3, 8.4.** `clay_mesh_load` / `clay_mesh_save`,
the mesher choice, and the merged export added in 0.27.0. FBX has a known
engine quirk ([#38](https://github.com/CyberdyneCorp/ClayCore/issues/38),
closed) worth re-checking on arrival.

**3. Diagnostics — 2.6, 10.11.** Backends discovered, backend active, why it
was chosen, engine revision, fallbacks this session. Small, and it is what
turns a bug report from this application into something actionable.

**4. Units and the working unit — 8.8.** Presentation-only switching.

**5. Bundles and attribution — 10.12, 10.13.** The macOS bundle and the Linux
distributable, and the attribution manifest the licence policy in `deny.toml`
is written against.

**6. Interface-thread instrumentation — 10.4.** A 16 ms threshold with the
operation responsible. Consolidation takes 6.4 s, so this has something real
to catch.

**7. LOD over brick mips — 3.9.** `read_bricks(lod)`, `build_mip` and
`current_lod` are all present.

### Armatures, as delivered

6.5 and 6.6 are done. Rigging follows ZBrush: drag out of a sphere to grow the
next, Alt to move a subtree, ⌘ to resize, mirrored authoring on by default,
and the scaffolding drawn only while the mode is on. Twelve spheres author in
roughly 0.4 s, so a single gesture costs about 34 ms — each edit rewrites the
armature node and refills the box it vacated, which is the price of a topology
the ABI will not let us edit in place.

The half that does not work is persistence of the *tree*. See the table above.

## What is slow and why

A dab costs 15.8 ms on a fresh document and 81 ms on the reference scene. The
difference is not the edit — the bricks re-meshed are identical — it is
[#73](https://github.com/CyberdyneCorp/ClayCore/issues/73). Measured at a fixed
80 bricks while the document grows from 1 node to 193:

| stage | 1 node | 193 nodes |
|---|---|---|
| `apply_stroke` + `mark_dirty` + `refill` | 0.6 ms | 0.6 ms |
| `clay_brick_cache_mesh`, normals **off** | 4.2 ms | 4.3 ms |
| `clay_brick_cache_mesh`, gradient normals | 4.8 ms | **120.1 ms** |

Marching is flat. Refill is flat. Only the gradient scales, and it scales with
the document rather than the region — the nodes in that measurement are at the
opposite pole from the bricks being meshed.

Two things could be done here without waiting:

**Host-side normals.** Meshing with `gradient_normals: false` and averaging
face normals takes the same measurement from 53 ms to 4 ms. Not taken, because
field gradients are better normals than face averages on a narrow band, and
the trade deserves a decision rather than a commit.

**Offer consolidation.** `clay_layer_consolidation_cost` already reports
`advises_consolidation: true` at 203 items, and consolidating takes a dab from
56 ms back to 13 ms. Nothing surfaces it. It costs 6.4 s on that layer, so it
belongs on a deliberate action, not mid-stroke. The specification requires it
never run unasked; it does not require us to keep quiet about it.

## Continuous integration

Seven jobs. Five green: Linux CPU-only, format/lint/audit, layering, OpenSpec
strict, and the performance gate. Two red, both on
[#71](https://github.com/CyberdyneCorp/ClayCore/issues/71): macOS CPU-only and
macOS Metal.

The performance gate compares against `benchmarks/baseline-macos-aarch64.json`
and fails on a regression. Budget breaches are printed but not enforced without
`--enforce-budgets`: the specification gates on a change *raising* latency, and
a gate that is red the day it is installed is one people learn to ignore.

## Open decisions

These change what gets built, and are better settled early than late. They are
task 11.1, and they gate archiving the change.

**The Dinâmica panel.** The design shows gravity at −9.81, rigidity and
damping. ClayCore has no solver and its roadmap proposes none. The panel
currently ships as voxel size and the multi-resolution level stack, which is
what "Dinâmica: Ligada" means operationally. Confirm that, or scope soft-body
simulation as its own research-sized change with no engine support behind it.

**Localisation scope.** The design is Portuguese throughout. Both pt-BR and
en-US are carried today, so the fallback path is exercised rather than assumed.
Shipping one or both is a product decision, not an architectural one.

**Default representation.** A new document currently opens SDF-first. Several
verbs exist on one representation only, so this decides what the first minute
of the application feels like.

**Whether to withhold the four bake-and-replace tools.** Suavizar, Relaxar,
Planar and Polir corrugate what they touch ([#67](https://github.com/CyberdyneCorp/ClayCore/issues/67)).
Withholding them on SDF layers would leave Planar and Polir with nowhere to
run, since they are SDF-only. Shipping a tool that damages a sculpt and
withdrawing two buttons are both defensible; doing neither by default is what
happens today.

## Known costs and escape routes

**The mesh upload is a full memcpy.** 2.7 ms at the current model size,
growing with the model rather than with the edit. `patch_vertices` would make
it incremental, and needs the weld problem solved first.

**The viewport meshes rather than raymarching bricks.** The volume path needs
no kernel math in a shader and takes meshing off the interaction path
entirely — which would also route around #73. It is the recorded escape route
if the latency budget comes under pressure, and it is what ClayCore's own
`docs/06` recommends for most hosts.

**GPU device injection is available and not taken.** It would remove the host
copy, but bypasses the brick cache — so adopting it means reimplementing the
dirty-set logic the cache provides. Not worth it while refill costs 0.6 ms.

**Our own crates are `publish = false`.** This is an application; its crates
are its internals. `cargo deny` enforces it, because a wildcard path
dependency on a publishable crate is a combination crates.io rejects.
