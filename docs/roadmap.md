# Roadmap

Where the project stands, what is left, and what is still undecided. Task
counts come from `openspec/changes/add-clayspace-desktop/tasks.md`, which is
the authority.

**106 of 109 tasks. Milestones 1 to 4 delivered; milestone 5 all but closed.**

Engine pinned at ClayCore **0.28.0**, at the tag rather than at `main` — the
tag is a release, `main` is where they are still working. A dab is 14.4 ms on a
bare document against a 50 ms budget, and 86 ms on the reference scene for a
reason that is upstream and understood; see *What is slow and why*. Startup to
first document is **11.6 ms**, down from 68.5.

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
| 2. Acceleration policy | M1 | 7/7 |
| 3. Rendering foundation | M2 | **8/9** — LOD blocked upstream |
| 4. MVVM skeleton | M3 | 7/7 |
| 5. Sculpting loop | M3 | 11/11 |
| 6. Masks and armatures | M5 | 6/6 |
| 7. Scene, layers and history | M4 | 13/13 |
| 8. Document lifecycle | M5 | 8/8 |
| 9. Interface shell and design system | M4 | 16/16 |
| 10. Performance and packaging | M5 | 13/13 |
| 11. Close-out | M5 | 1/3 |

## What is blocked, and what is not

**One task is blocked by ClayCore: 3.9, level of detail.** Everything else on
the list is either done or waiting on a decision. I had 3.9 down as unblocked
on the last roadmap, on the grounds that `build_mip`, `current_lod` and
`read_bricks(lod)` all exist. They do — and `clay_brick_cache_mesh` takes no
level, so a mip can be built and read and not meshed. A coarse viewport would
mean reimplementing the mesher this application deliberately does not own.
`claycore_lod.rs` records it and fails when the meshing call grows a level.

Every other open upstream issue degrades something already built rather than
stopping something next. CI is the exception, where the macOS rows cannot pass
until the engine compiles on the CI compiler.

### Taken up in 0.28.0

Five issues filed from this work were released, and each one flipped a test
here rather than being noticed by reading a changelog. That is the mechanism
working: the repro that documents a defect fails the day it stops being one.

| Issue | What it changed here |
|---|---|
| [#60](https://github.com/CyberdyneCorp/ClayCore/issues/60) layer mirror had no effect | **X symmetry is on by default**, as the design always asked. Costs latency — see below |
| [#61](https://github.com/CyberdyneCorp/ClayCore/issues/61) `CLAY_OP_ADD` ignored stroke strength | **Intensidade works** on every tool, not just the Relief-based ones. No code change: the slider was already mapped to `strength` |
| [#62](https://github.com/CyberdyneCorp/ClayCore/issues/62) relief amplitude undocumented | Nothing to undo; the mapping was already right, and is now checkable |
| [#64](https://github.com/CyberdyneCorp/ClayCore/issues/64) Metal 7–10× slower at refill | **Refill routes to the GPU above 16 bricks.** Startup to first document went 68.5 ms → 11.6 ms |
| [#66](https://github.com/CyberdyneCorp/ClayCore/issues/66) subset meshing omitted straddlers | **The whole-surface `settle` after every stroke is gone.** An incremental sync is now triangle-for-triangle what a rebuild produces |
| [#67](https://github.com/CyberdyneCorp/ClayCore/issues/67) bake-and-replace corrugated | **The four damaging tools stopped damaging.** `clay_volume_params.feather` crossfades the replace; measured roughness 7.00 → 5.00 against a 4.88 baseline |

### Upstream: fixed on `main`, not in 0.28.0

Merged after the tag. We are pinned to the release, so these are still ahead of
us — and one of them is the CI blocker.

| Issue | What it would unblock |
|---|---|
| [#71](https://github.com/CyberdyneCorp/ClayCore/issues/71) AppleClang | The three macOS CI jobs |
| [#73](https://github.com/CyberdyneCorp/ClayCore/issues/73) gradient normals scale with the document | The dab latency, which symmetry has just tripled |
| [#69](https://github.com/CyberdyneCorp/ClayCore/issues/69) no layer enumeration | Layer names, visibility and stack order across a reload |

### Upstream: still open

| Issue | Effect here | Our position |
|---|---|---|
| [#93](https://github.com/CyberdyneCorp/ClayCore/issues/93) no LOD meshing | **Blocks the last of 3.9.** Mips build and read; nothing meshes them | Filed. The host halves are built — policy and maintenance — so this is one call away |
| [#91](https://github.com/CyberdyneCorp/ClayCore/issues/91) no node enumeration | Finding a reloaded rig needs an id probe | Filed. A *checkable* probe, unlike the old layer one |
| [#92](https://github.com/CyberdyneCorp/ClayCore/issues/92) no layer rename | A rename is lost on save | Filed. Visible only now that names read back |
| [#73](https://github.com/CyberdyneCorp/ClayCore/issues/73) gradient normals scale with document size | A dab is 4 ms on a fresh document and 120 ms after 192 edits, and symmetry doubles the region | **Fixed on `main`, not in 0.28.0.** Workaround available: host-side normals. Not taken |
| [#69](https://github.com/CyberdyneCorp/ClayCore/issues/69) no layer enumeration | A reopened document loses layer names, visibility and **stack order** | Layer ids recovered by probing. Order loss is a silent correctness difference |
| [#77](https://github.com/CyberdyneCorp/ClayCore/issues/77) a placed armature is write-only | A saved rig comes back as surface only, so it cannot be re-posed or edited | Verified in `claycore_armature_readback.rs`: `clay_layer_stroke_points` refuses the primitive, and nothing reads the parent array. The armature-shaped instance of #16 |
| [#63](https://github.com/CyberdyneCorp/ClayCore/issues/63) Metal absent on paravirtual GPUs | None for us | Not ours; kernel half fixed in 0.27.3 |

## What is left

**3.9, level of detail.** Blocked upstream; see above.

**11.1, the open decisions.** Four of them, below. They gate archiving the
change and nobody but the product owner can settle them.

**11.3, archive.** After 11.1.

That is the whole list. Everything else in milestone 5 landed: masks and
armatures, document lifecycle including autosave and recovery, mesh import and
export, diagnostics, units, instrumentation, bundles and attribution, backend
parity and the cross-platform document check.

### Armatures, as delivered

6.5 and 6.6 are done. Rigging follows ZBrush: drag out of a sphere to grow the
next, Alt to move a subtree, ⌘ to resize, mirrored authoring on by default,
and the scaffolding drawn only while the mode is on. Twelve spheres author in
roughly 0.4 s, so a single gesture costs about 34 ms — each edit rewrites the
armature node and refills the box it vacated, which is the price of a topology
the ABI will not let us edit in place.

The half that does not work is persistence of the rig itself. A placed
armature cannot be read back at all — not the parents, and not the positions
and radii either. See the table above.

## What is slow and why

Sculpting is not what costs. Splitting one segment after 96 edits, over 80
bricks:

| stage | cost |
|---|---|
| the edit — stroke, mark dirty, refill | 1.09 ms |
| mesh, face normals | 7.71 ms |
| mesh, gradient normals | 83.22 ms |

91% of a segment was one call's gradient-normal term —
[#73](https://github.com/CyberdyneCorp/ClayCore/issues/73), fixed on ClayCore's
`main` and not in 0.28.0. So the drag no longer pays it: `sync` shades with
face normals and `SurfaceGeometry::refine` buys the gradient back over the
gesture's own keys when the pointer comes up. A dab went 86 ms → 12 ms.

The underlying scaling is unchanged, and it is still worth fixing upstream —
the release pass pays it, and it grows with the document. Measured at a fixed
80 bricks while the document grows from 1 node to 193:

| stage | 1 node | 193 nodes |
|---|---|---|
| `apply_stroke` + `mark_dirty` + `refill` | 0.6 ms | 0.6 ms |
| `clay_brick_cache_mesh`, normals **off** | 4.2 ms | 4.3 ms |
| `clay_brick_cache_mesh`, gradient normals | 4.8 ms | **120.1 ms** |

Marching is flat. Refill is flat. Only the gradient scales, and it scales with
the document rather than the region — the nodes in that measurement are at the
opposite pole from the bricks being meshed.

One of these was taken in 0.28.0 and one still could be:

**Host-side normals.** Meshing with `gradient_normals: false` and averaging
face normals takes the same measurement from 53 ms to 4 ms. Not taken, because
field gradients are better normals than face averages on a narrow band, and
the trade deserves a decision rather than a commit.

**Symmetry triples the keys a segment re-meshes** — 170 to 526 for one dab,
because each half is dilated by a ring of its own. That is now affordable; it
was not before the shading split.

**Offer consolidation.** `clay_layer_consolidation_cost` already reports
`advises_consolidation: true` at 203 items, and consolidating takes a dab from
56 ms back to 13 ms. Nothing surfaces it. It costs 6.4 s on that layer, so it
belongs on a deliberate action, not mid-stroke. The specification requires it
never run unasked; it does not require us to keep quiet about it.

## Continuous integration

Eleven jobs. Green: Linux CPU-only, Linux Vulkan, format/lint/audit, layering,
packaging, OpenSpec strict, the performance gate, and the Linux half of the
document-bytes matrix. Red on
[#71](https://github.com/CyberdyneCorp/ClayCore/issues/71): macOS CPU-only,
macOS Metal, and the macOS half of document-bytes — which also holds up the
job that compares the two platforms' digests.

The performance gate compares against `benchmarks/baseline-macos-aarch64.json`
and fails on a regression. Budget breaches are printed but not enforced without
`--enforce-budgets`: the specification gates on a change *raising* latency, and
a gate that is red the day it is installed is one people learn to ignore.

## Open decisions

These change what gets built, and are better settled early than late. They are
task 11.1, and they gate archiving the change.

**~~The Dinâmica panel.~~ Settled: out of scope.** The design shows gravity,
rigidity and damping; ClayCore has no solver and proposes none, so the panel
ships as voxel size and the resolution stack. Soft-body simulation is not
being scoped as part of this change.

**~~Localisation scope.~~ Settled: three locales.** en-US, pt-BR and es-419
(Latin American Spanish, to pair with Brazilian Portuguese). All three are
carried and each gets a rendered capture.

**The Dinâmica panel — superseded, kept for the record.** The design shows gravity at −9.81, rigidity and
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

**~~Whether to withhold the four bake-and-replace tools.~~ Settled by 0.28.0.**
They corrugated what they touched; the feathered replace fixed it, and all four
now leave the clay beside the stroke alone. What is left is not a decision but
a matter of character: Suavizar and Relaxar are subtle, because relax moves the surface by
less than a cell per pass and the cache's cell is 0.02.

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
