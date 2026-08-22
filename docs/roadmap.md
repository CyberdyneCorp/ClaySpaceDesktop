# Roadmap

Where the project stands, what is left, and what is still undecided. Task
counts come from `openspec/changes/add-clayspace-desktop/tasks.md`, which is
the authority.

**107 of 109 tasks. Milestones 1 to 4 delivered; milestone 5 all but closed.**

Engine pinned at ClayCore **0.30.0**, at the tag rather than at `main` — the
tag is a release, `main` is where they are still working. On the reference
scene a dab is 12.2 ms median against a 50 ms budget and startup to first
document is 15.1 ms, both recorded against 0.29.1 on macOS aarch64; the 0.30.0
pin has not been re-measured on that machine. See *What is slow and why*.

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
| 3. Rendering foundation | M2 | 9/9 |
| 4. MVVM skeleton | M3 | 7/7 |
| 5. Sculpting loop | M3 | 11/11 |
| 6. Masks and armatures | M5 | 6/6 |
| 7. Scene, layers and history | M4 | 13/13 |
| 8. Document lifecycle | M5 | 8/8 |
| 9. Interface shell and design system | M4 | 16/16 |
| 10. Performance and packaging | M5 | 13/13 |
| 11. Close-out | M5 | 1/3 |

## What is blocked, and what is not

**Nothing is blocked by ClayCore any more.** 3.9, level of detail, was the last
one: `build_mip`, `current_lod` and `read_bricks(lod)` had always existed, and
`clay_brick_cache_mesh` took no level, so a mip could be built and read and not
meshed. ClayCore 0.30.0 added `clay_brick_cache_mesh_lod` (#93) and 3.9 closed
on it — see *Level of detail, as delivered*. What is left on the list is
waiting on a decision rather than on an engine.

There are no open upstream issues left either, and nothing released is waiting
on host code. Every issue filed from this work has been released and taken up —
including [#71](https://github.com/CyberdyneCorp/ClayCore/issues/71), which was
the macOS CI blocker.

### Released in 0.28.0

Six issues filed from this work were released, and each one flipped a test
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

### Released in 0.29.x and 0.30.0

Taken up here, each one flipping a test rather than being read about.

| Issue | Released | What it changed here |
|---|---|---|
| [#69](https://github.com/CyberdyneCorp/ClayCore/issues/69) no layer enumeration | 0.29.0 | **A reopened document keeps its layers as they were** — names, visibility and stack order, the last of which was a silent correctness difference. `document_io.rs` |
| [#71](https://github.com/CyberdyneCorp/ClayCore/issues/71) AppleClang | 0.29.0 | The engine compiles on the CI compiler, which was the macOS CI blocker |
| [#73](https://github.com/CyberdyneCorp/ClayCore/issues/73) gradient normals scale with the document | 0.29.0 | With [#83](https://github.com/CyberdyneCorp/ClayCore/issues/83) in 0.29.1, the gradient went from 11× the cost of face normals to under 1.5×. A dab is 12 ms |
| [#77](https://github.com/CyberdyneCorp/ClayCore/issues/77) a placed armature is write-only | 0.29.0 | **A saved rig comes back posable** — the tree and its topology, not just the skin. `armature_persistence.rs` |
| [#93](https://github.com/CyberdyneCorp/ClayCore/issues/93) no LOD meshing | 0.30.0 | **Task 3.9 closed.** See *Level of detail, as delivered* |
| [#91](https://github.com/CyberdyneCorp/ClayCore/issues/91) no node enumeration | 0.30.0 | **A reloaded rig is found by enumeration, not by probing ids.** The old scan gave up after sixteen consecutive misses; gaps survive a round trip and are unbounded, so a document whose ids ran `[1, 32..42]` reopened reporting no armature at all. `layer_nodes.rs` |
| [#92](https://github.com/CyberdyneCorp/ClayCore/issues/92) no layer rename | 0.30.0 | **A rename is saved.** One command, so one undo step. `layer_rename.rs` |
| [#99](https://github.com/CyberdyneCorp/ClayCore/issues/99) armature has one op per item | 0.30.0 | **Negative ZSpheres are real.** The sign belongs to the node, so the membrane along its links is cut, the sign survives a reload, and a negative may carry a limb. The rig is one item again. `armature_signs.rs` |

### Upstream: released, not yet taken up here

**Nothing.** Every issue filed from this work has been released *and* taken up.

### Upstream: available and not needed

| Issue | Released | Why it does not change anything here |
|---|---|---|
| [#63](https://github.com/CyberdyneCorp/ClayCore/issues/63) partial backend registration | 0.30.0 | A backend that loses one pipeline now registers and says what it lost, and `clay_backend_supports` / `clay_backend_diagnostic` answer why one is missing. Worth surfacing in the diagnostics report; nothing is broken without it |
| [#86](https://github.com/CyberdyneCorp/ClayCore/issues/86) whole-grid voxel meshing | 0.30.0 | An incremental voxel display path, mirroring the brick cache's. The voxel tools here do not re-mesh per frame, so nothing pays the cost it removes |

**There are no open upstream issues affecting this project.** Every one filed
from this work has been released.

## What is left

**11.1, the open decisions.** Four of them, below. They gate archiving the
change and nobody but the product owner can settle them.

**11.3, archive.** After 11.1.

That is the whole list. Everything else in milestone 5 landed: masks and
armatures, document lifecycle including autosave and recovery, mesh import and
export, diagnostics, units, instrumentation, bundles and attribution, backend
parity and the cross-platform document check.

### Level of detail, as delivered

3.9. A model past three extents from the camera drops to the brick cache's
mips; inside two and a half it comes back. The distance at which detail is
dropped is further out than the one at which it is restored, and that band is
the whole design — a single threshold would swap the entire surface every time
a resting camera twitched. A model under 2048 surface bricks is never
coarsened, because it meshes inside a frame anyway and coarsening it only makes
it visibly worse.

Three things the coarse path does deliberately:

- **It is face-shaded.** Level 1 refuses gradient normals rather than
  downgrading them — a coarse vertex sits on the mip's surface rather than the
  field's, where a per-brick culled tape and the whole document's stop
  agreeing. `claycore_lod.rs` pins that refusal, because the host draws face
  normals *because* of it.
- **It falls back rather than failing.** A coarse key with no mip is refused by
  the engine, and one dirty child is enough, so the adapter hands over only the
  keys that have one. With none, the request draws the full surface: slow beats
  empty. The end of a gesture asks again, once the mips are up.
- **An edit returns to full resolution.** The two levels do not share a key
  space, and dirtying any child drops its mip, so there is nothing coarse left
  to draw where the edit landed. `lod_switching.rs` holds this one.

Switching level is a full re-mesh. It is affordable only because the hysteresis
band makes it rare; incremental syncing happens at full resolution only.

On the reference form the drop takes 283,612 triangles to 86,130, and the share
of the frame that can tell falls from 2.1% filling the screen to 1.3% at the
distance it actually drops at. Two things the coarse surface does not do well,
both looked at rather than assumed — see `visual_lod.rs` and the
[known-degraded table](features.md#known-degraded):

- It is faintly speckled, because level 1 forces face normals and degenerate
  triangles shade badly under them. Not an LOD defect: level 0 with face
  normals speckles identically, and with gradient normals it does not.
- It has no mip for coarse blocks on the edge of the surface band, because one
  needs all eight children evaluated and the cache evaluates only surface
  bricks — 70 of 242 on the reference form, and no amount of settling fixes it.
  Filling those with full-resolution geometry moves 0.69% of the frame and was
  not taken.

### Armatures, as delivered

6.5 and 6.6 are done. Rigging follows ZBrush: drag out of a sphere to grow the
next, Alt to move a subtree, ⌘ to resize, mirrored authoring on by default,
and the scaffolding drawn only while the mode is on. Twelve spheres author in
roughly 0.4 s, so a single gesture costs about 34 ms — each edit rewrites the
armature node and refills the box it vacated, which is the price of a topology
the ABI will not let us edit in place.

Persistence works since ClayCore 0.29.0 closed
[#77](https://github.com/CyberdyneCorp/ClayCore/issues/77): a saved rig comes
back with its tree and topology, not just its skin, so a reopened document can
be posed. `armature_persistence.rs` and `claycore_armature_readback.rs` hold
both halves.

The negative ZSphere is real since 0.30.0 (#99). It used to be a separate
subtractive item placed beside the rig, which carved a ball but left the
membrane along its links drawn, lost the sign on reload, and forced negatives
to be leaves. The sign is now the node's own, so all three go away and the rig
is a single item again — `armature_signs.rs` holds each of the three.

## What is slow and why

Sculpting is not what costs. Splitting one segment after 96 edits, over 80
bricks:

| stage | cost |
|---|---|
| the edit — stroke, mark dirty, refill | 1.09 ms |
| mesh, face normals | 7.71 ms |
| mesh, gradient normals | 83.22 ms |

91% of a segment was one call's gradient-normal term —
[#73](https://github.com/CyberdyneCorp/ClayCore/issues/73), since fixed in
0.29.0 and narrowed again by
[#83](https://github.com/CyberdyneCorp/ClayCore/issues/83) in 0.29.1. A dab
went 86 ms → 12 ms.

The table above is the 0.28.0 measurement, kept because it is what the shading
split was built against. The gradient has since come down a long way over a
fixed 80-brick sample:

| engine | face normals | gradient normals | premium |
|---|---|---|---|
| 0.28.0 | 7.7 ms | 83.2 ms | 11x |
| 0.29.1 | 8.0 ms | 11.5 ms | 1.4x |
| 0.30.0 | 12.6 ms | 13.2 ms | 1.04x |

That last row read as "the gradient is free now", and for one release the drag
shaded fully and there was no second pass. The end of a gesture went 17.5 ms →
2.0 ms, which was real: re-shading the 111 keys a stroke touches had cost
15.7 ms at pointer-up, and the application said so on every stroke — `a
interface travou: sombreamento final 17 ms`, with the `stroke` stall printed
beside it being the same event.

But 80 bricks is not what a segment meshes. Over the 27 keys a dab dirties, the
premium is not 1.04x:

| shading | median | p95 | worst |
|---|---|---|---|
| face normals | 3.4 ms | 4.0 ms | 6.2 ms |
| gradient | 4.9 ms | 8.1 ms | **18.9 ms** |

40% at the median, and in the tail the difference between a segment that always
fits a frame and one that sometimes takes 19 ms. The brush ring is drawn in the
frame that meshes the edit, so those spikes are the ring visibly trailing the
pointer — reported as exactly that, and the reason the split is back.

It is not the old split. The drag shades fast, and `SurfaceGeometry::refine_within`
pays the gradient back **a segment at a time, on frames that are not sculpting**
— a pause mid-gesture or the frames after the pointer lifts, each bounded by
what the frame has left. A gesture's debt clears in about five idle frames.
Neither end pays a hitch: the worst mid-drag segment is 4.9 ms and pointer-up is
2.0 ms, and `gesture_end.rs` fails if either leaves a frame. A third test there
holds the drained surface triangle-for-triangle against a full rebuild, which is
what says the queue can be drained in pieces at all.

The scaling below is the shape of the original problem, also measured on
0.28.0, at a fixed 80 bricks while the document grows from 1 node to 193:

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
document-bytes matrix.

The four macOS rows — CPU-only, Metal, the macOS half of document-bytes, and
the digest comparison that waits on it — were red on
[#71](https://github.com/CyberdyneCorp/ClayCore/issues/71), which **shipped in
0.29.0**. Nothing here has confirmed them green since: it takes a run on the
macOS runners, and the pin moved two releases without one. Treat them as
unknown rather than as blocked.

The performance gate compares against `benchmarks/baseline-macos-aarch64.json`
and fails on a regression. Budget breaches are printed but not enforced without
`--enforce-budgets`: the specification gates on a change *raising* latency, and
a gate that is red the day it is installed is one people learn to ignore.

## Open decisions

These change what gets built, and are better settled early than late. They are
task 11.1, and they gate archiving the change.

**One is still open.**

**Default representation — open.** A new document currently opens SDF-first.
Several verbs exist on one representation only, so this decides what the first
minute of the application feels like. Nobody but the product owner can settle
it, and 11.3 waits on it.

The other three are settled, kept here because the reasoning is what makes them
stay settled:

**~~The Dinâmica panel.~~ Out of scope.** The design shows gravity at −9.81,
rigidity and damping. ClayCore has no solver and its roadmap proposes none, so
the panel ships as voxel size and the multi-resolution level stack — which is
what "Dinâmica: Ligada" means operationally. Soft-body simulation would be its
own research-sized change with no engine support behind it.

**~~Localisation scope.~~ Three locales.** en-US, pt-BR and es-419 (Latin
American Spanish, to pair with Brazilian Portuguese). All three are carried and
each gets a rendered capture, so the fallback path is exercised rather than
assumed.

**~~Whether to withhold the four bake-and-replace tools.~~ Settled by 0.28.0.**
They corrugated what they touched; the feathered replace fixed it, and all four
now leave the clay beside the stroke alone. What is left is not a decision but
a matter of character: Suavizar and Relaxar are subtle, because relax moves the
surface by less than a cell per pass and the cache's cell is 0.02.

## Known costs and escape routes

**~~The mesh upload is a full memcpy.~~ Taken.** It was 2.7 ms and grew with
the model rather than with the edit. Each key now owns a span of both buffers
and keeps it (`clayspace-app/src/slots.rs`), so a dab writes the spans it
touched through `GpuMesh::patch_vertices` — 0.1 ms of a 5.2 ms dab, measured by
`dab_profile.rs`, which fails if the upload ever dominates again.

**The viewport meshes rather than raymarching bricks.** The volume path needs
no kernel math in a shader and takes meshing off the interaction path
entirely — which would also route around #73. It is the recorded escape route
if the latency budget comes under pressure, and it is what ClayCore's own
`docs/06` recommends for most hosts.

**GPU device injection is available and not taken.** It would remove the host
copy, but bypasses the brick cache — so adopting it means reimplementing the
dirty-set logic the cache provides. Still not worth it: the copy it would
remove is 0.1 ms of a 5.2 ms dab.

**Where a dab actually goes**, measured on this machine at 27 keys and 285,712
triangles (`dab_profile.rs`):

| stage | cost | share |
|---|---|---|
| engine: `apply_stroke` + refill | 1.8 ms | 36% |
| engine: brick cache mesh | 2.9 ms | 55% |
| ours: copy into the vertex layout | 0.1 ms | 1% |
| ours: split into per-key geometry | 0.4 ms | 7% |
| ours: write the changed spans | 0.1 ms | 1% |

**91% of a dab is inside ClayCore**, and drawing the result is 0.45 ms a frame
on top. Host-side rendering work is not where the remaining time is.

**~~The refill crossover is a constant measured on one machine.~~ Taken.** It
was `GPU_CROSSOVER_BRICKS = 16`, measured on an M-series Mac, and it decided
for every machine. On a 24-thread Linux box with an RTX 5060 both accelerated
backends lose to the CPU at every batch size — CUDA by 3.5x, Vulkan by 2.5x —
so a dab and the whole startup fill went to a backend that could not win, and
startup cost **179 ms against 63 ms**. The constant is now the floor and the
starting guess; above it the first large refill of a session calibrates the two
backends against each other and every refill after that is timed. `dab.median`
did not move, because a dab is dominated by meshing rather than refill.

**Our own crates are `publish = false`.** This is an application; its crates
are its internals. `cargo deny` enforces it, because a wildcard path
dependency on a publishable crate is a combination crates.io rejects.
