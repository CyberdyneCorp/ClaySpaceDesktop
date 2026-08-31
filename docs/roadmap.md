# Roadmap

Where the project stands, what is left, and what is still undecided. Task
counts come from the tasks files under `openspec/changes/`, which are the
authority.

**Seven changes.** `add-clayspace-desktop` stands at **107 of 109 tasks** —
milestones 1 to 4 delivered, milestone 5 all but closed.
`make-representations-first-class` is **complete**, and is what took the
application from one vocabulary to three; its own summary is below.
`place-and-transform-objects` is **complete**: a shape placed in the scene
stays an addressable item, with a manipulator on it and a boolean that follows
it. `benchmark-every-operation` stands at **45 of 46** — the table went from
one brush to every brush, operation, conversion and bake.
`upgrade-engine-0-52-2` stands at **12 of 13**, and both remainders are the
same task: re-recording the macOS baseline, which needs a macOS machine.
`subtools` stands at **37 of 38**: a scene is now a list of separate forms,
each activated by clicking it, each with its own mask, symmetry and rig, each
movable whole, and any two of them resolvable into a third by a boolean.
Switching onto a carried mesh subtool used to hold the interface thread for
160 ms against a 16 ms bound; the document holds a sculptor per mesh now
instead of one, and it is 0.00 ms — the last open budget in the suite, closed.
Its one remainder — the per-layer mask reaching the engine's own — is closed by
`close-brush-integration-gaps`, which also bound the brush verbs the engine had
and the shelf did not reach: a brush colour and a Pintar that changes a pixel,
Mover and Planar on a grid, Argila and Vinco on a field, and Mover Topológico
as a tool of its own.

Engine pinned at ClayCore **0.60.0**, at the tag rather than at `main` — the
tag is a release, `main` is where they are still working. On the reference
scene a dab is 2.1 ms median against a 50 ms budget and startup to first
document is 11.4 ms, recorded against 0.52.2 on Linux x86_64. The macOS
baseline still reads 0.29.1 — 12.2 ms and 15.1 ms there — and nothing since
has been re-measured on that machine. See *What is slow and why*.

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

## The second change: three representations, first-class

`make-representations-first-class` is complete and ready to archive. It began
as a shell that followed the active layer and grew into most of what the
application now offers beyond the first change's sculpt loop.

| Group | What it delivered |
|---|---|
| 1–2 | A capability table per representation, and a shell that follows the active layer rather than showing one list mostly greyed out |
| 3–4 | Conversions between SDF, voxel and mesh, each stating what it costs first |
| 5 | Mesh sculpting: the engine's sixteen fixed-topology brushes |
| 6–7 | The voxel vocabulary, repair, and recorded sculpt passes |
| 8 | The SDF vocabulary, blend profiles and the combine list |
| 9–10 | Close-out, and what checking the work turned up |
| 11 | Crossing into a mesh |
| 12–13 | The polyframe, and what Move actually does — measured against Blender |
| 14–15 | The keys a sculptor holds, and masking on the key and on the screen |
| 16 | The mask menu, entry by entry |
| 17 | The deformation cage and its manipulator |
| 18 | Symmetry on the two representations that had none |
| 19 | Three languages, and a way to choose between them |
| 20–21 | Every SDF and voxel brush: it works, it takes a sign, it mirrors |
| 22 | A voxel layer drawn as its form rather than as boxes |
| 23–24 | The tendril a snakehook pulls, and a tube along a curve |
| 25 | Zooming at the clay rather than through it |
| 26–27 | Reference images behind the form, in PNG and JPEG, and the clay's own opacity |

A recurring shape across those groups is worth recording, because it will
recur: **the engine already had both halves of a pair and only one was ever
bound.** `FlattenMode::FillOnly`, `sculpt_inflate(-1)`, `sculpt_magnify`,
`clay_voxel_mask_extrude`, `clay_mesh_lattice_displacement`,
`clay_item_set_curve_points`, `clay_layer_set_stroke_points` and
`clay_layer_lattice_gizmo` were all present in ClayCore and unreachable from
here. The second recurring lesson is about measurement: **choose an instrument
that can see the thing being asked about** — silhouette rather than colour,
form rather than vertex counts, cells rather than pixels, displacement rather
than silhouette.

## What is blocked, and what is not

**Nothing is blocked by ClayCore any more.** 3.9, level of detail, was the last
one: `build_mip`, `current_lod` and `read_bricks(lod)` had always existed, and
`clay_brick_cache_mesh` took no level, so a mip could be built and read and not
meshed. ClayCore 0.30.0 added `clay_brick_cache_mesh_lod` (#93) and 3.9 closed
on it — see *Level of detail, as delivered*. What is left on the list is
waiting on a decision rather than on an engine.

Seven upstream findings are open, and none of them blocks anything. Five carry
numbers, and three of those were filed from the subtools work.

[#321](https://github.com/CyberdyneCorp/ClayCore/issues/321) — **a layer carries
no combine operation.** The document composes its layers by hard union
(`clay/scene/tape.h`), so there is no way to say that one layer *subtracts* from
another. This is what a **live** subtool boolean waits on. What is built instead
is a *resolved* one: each operand is sampled into a volume, the two are combined
in a subtool of its own, and moving an operand afterwards does not update the
result. The interface says so rather than implying otherwise, and the operands
are kept so the operation can be run again from a new position. When #321 lands,
the same vocabulary upgrades to a live boolean without a word of it changing —
see [features.md](features.md#a-boolean-between-two-subtools).

[#210](https://github.com/CyberdyneCorp/ClayCore/issues/210) — `clay_document_undo`
does not report what it changed, so an undo has to dirty the whole layer. Undo
works and is correct; it costs far more than the edit it reverses. See *Undo,
which costs far more than the edit it takes back*. Subtools made it cost a
little more again: solo has no journal pause to hide behind, so its visibility
commands land on the engine's stack and undo steps over them before it reaches
the edit underneath — measured, a ⌘Z after a released solo on a three-subtool
document is 203 ms against the 87 ms an undo costs alone, and every millisecond
of the difference is a hop paying #210's whole-layer refill.

[#364](https://github.com/CyberdyneCorp/ClayCore/issues/364) — **instance layers
are specified and have no constructor.** The header describes a layer that
shares another's content under its own transform; nothing in the ABI creates
one. This is what a *cheap* duplicate waits on. What is built instead is an
honest copy: the source is sampled into a volume of its own, so sculpting the
copy cannot reach the original, and the control says **copiar** rather than
naming something this cannot do. Measured, a copy of the reference form is
4.3 s — the whole of which is the sampling an instance would not do. Filed from
this work.

[#365](https://github.com/CyberdyneCorp/ClayCore/issues/365) — **a voxel grid is
reachable only by name.** `clay_document_voxel_layer` takes a string, so two
layers sharing a name shadow each other's grid and a stroke lands on the wrong
one. Harmless while a document held one grid; a scene of subtools is exactly
where two layers come to share a name. Until it lands, every insertion derives a
unique default name — `unique_layer_name`, which the mesh import and the stack's
add control go through as well, since a collision made after the fact shadows a
grid just as surely. Filed from this work.

[#368](https://github.com/CyberdyneCorp/ClayCore/issues/368) — **a mesh
sculptor cannot be built off the interface thread.** `clay_mesh_sculptor_create`
is a weld and an adjacency pass — 160 ms over the reference form's 296,216
triangles — and a mesh layer has no other route to its surface, since the pick
that follows an activation is answered by `clay_mesh_sculptor_raycast`. Holding
a sculptor per mesh took the *repeated* cost out; what is left is the first weld
of each mesh, and it has nowhere to go on this side. The call resolves its mesh
through a mutable path into the document and the ABI's only threading contract
is the brick cache's, so the ask is either that contract extended to this call
or a split between an off-thread adjacency build and a cheap adopt. Filed from
this work. See *Subtools: what switching costs*.

[#394](https://github.com/CyberdyneCorp/ClayCore/issues/394) — **`clay_item_set_gate`
is accepted and inert.** The entry point that would make
a mask protect a surface from an *operation* rather than only from a brush — the
engine's own note says "a mask over an ear has never done anything about the
next boolean. This does." Measured against 0.39.0, it does not: with a mask
sampling 1.0 at the cut's own centre and 65,752 cells painted, a subtracting
edit takes the protected region at every width and threshold tried, and the call
never refuses. The contract promises one or the other — "a gate that protects
nothing and reports success is harder to notice than a failure". The wrapper is
written and matches the contract; the application does not call it, because a
call per stroke that does nothing is a cost with no benefit and a promise in the
interface that would not be kept. `claycore/tests/mask_gate.rs` is a tripwire
written to fail when the engine honours it, and names `stroke_sdf` as where the
call goes back.

**A placed node's transform, parameters and operation could be set and never
read** — [#317](https://github.com/CyberdyneCorp/ClayCore/issues/317), and as
of the 0.60.0 pin they can be read: `clay_layer_node_transform`,
`clay_layer_node_transform_nonuniform`, `clay_layer_node_params` and
`clay_layer_node_op_blend` are all declared in
`vendor/ClayCore/bindings/c/clay.h` and generated into `claycore-sys`. So the
gap below is closed in the engine and still open here — the sidecar table in
`clayspace_engine::objects` is now a workaround for a limitation that no
longer exists, and comes out in its own change, minus a colour column: colour
stays write-only, deliberately, and the release says so.

`clayspace-app/tests/claycore_repros.rs` was supposed to be what said the day
this became true, and it did not: its
`a_placed_node_reports_its_primitive_and_nothing_else` asserts what *can* be
read and never that the readers are absent, so it passes on both sides of the
change. A tripwire that cannot fail is worth knowing about; the two in
`crates/claycore/tests` — `mask_gate.rs` and `alpha_deformer.rs` — are written
the other way and both still hold, so the mask gate is still accepted and
still does not protect.

`clay_layer_set_transform`, `clay_layer_set_prim` and
`clay_layer_set_op_blend` write them; nothing reads any of them back. What can
be read is `clay_layer_node_prim` — which primitive a node carries — and its
own note gives the reload model it belongs to: "ask what the node is, then call
the reader that applies". The readers that apply exist for an armature and for
a stroke's points. There is none for a plain item, and no host-data channel in
the document to keep one in.

This is what makes placed objects keep a table beside every document the
application saves: an object's size, where it stands and how it combines are
things only this application knows, so a reopened document could otherwise show
a box and nothing else about it. `clay_layer_node_influence_bound` is a partial
answer and is used as one — it is where an object's *dirty region* comes from —
but it is dilated by rounding and blend support and, under a layer mirror,
covers the reflection too, so it is not a position. Measured: a 0.4 sphere
scaled by 1.25 reports a box 1.0 wide, and an object placed at 0.9 in a
mirrored layer reports its bound centred at the origin.

Asked for as `clay_layer_node_transform`, `clay_layer_node_params` and
`clay_layer_node_op_blend`, which is #317. `clayspace-app/tests/claycore_repros.rs`
holds the gap as it stands today and fails when it closes, which is when
`clayspace_engine::objects`'s table comes out rather than being adapted. That
the issue is closed and the readers are absent from the pinned header is the
reason the tripwire is a test rather than a note: a changelog says a thing is
done, and a test says whether this build can call it.

[#392](https://github.com/CyberdyneCorp/ClayCore/issues/392) — **a stroke's
template alpha is not resolved into each stamp's frame.**
`clay_layer_apply_stroke` documents its item as "the stamp template scaled to
each stamp's radius", and `clay_item_add_alpha` puts the stamp's centre, extent
and radius in the *item's own* space — so a caller places the alpha on the
template and expects it at every stamp. It is not resolved there. Measured as
the rise along a stroke, swept over the stamp's centre: `[0, 0, 0]`, `[0, 0,
0.2]` and `[0, 0, 0.35]` — every sensible place on a 0.35 template — move the
surface by nothing, while `[0, 0, 0.7]` and `[0, 0, 1.0]`, which mean nothing
in that frame and correspond to where the *body's* surface sits in the world,
lift the whole path roughly evenly rather than leaving the mark the stamp
carries. So alpha stamps work on voxels and on meshes, which take one by their
own routes, and an SDF *stroke* states the refusal rather than passing an alpha
that would land somewhere else.

**`claycore/tests/alpha_deformer.rs` proved this with the wrong variable until
#392 was written**, and the correction belongs here because it is the second
tripwire-that-cannot-fail this file has had to record. It swept the amplitude
with a zero alpha `direction`, which that entry point documents as the normal
of the stamp's plane with no all-zeroes fallback — so it measured a degenerate
plane and would have gone on passing after the engine fixed the thing it was
written to catch. The reading was right; the evidence was not. It now sweeps
the centre and calibrates against the misplaced case in the same run, and a
mutation confirms it fails when the gap closes.

**The per-layer mask is reached, and it was never an upstream gap.** It sat
here for a while as one — the engine writes a mask with the document and
`claycore`'s masking surface could not reach it — and the obstacle was on this
side the whole time: a document-owned mask is lent *out* of the document, and
all five masked verbs wanted the document and the mask together, which Rust
cannot spell. `claycore::MaskSource` names the mask's **layer** instead of
lending the handle, `Document::layer_mask` lends one through a shared borrow,
and `Document::voxel_layer_masked` hands over a grid and its layer's mask out
of one borrow. Masks now survive a save and a reopen and record on the engine's
history. See [features.md](features.md#masking).

[#391](https://github.com/CyberdyneCorp/ClayCore/issues/391) —
**`clay_layer_move_surface` has no counterpart for the radial scale.** The
engine's field pinch and magnify is `CLAY_DEFORM_MAGNIFY`, one signed strength,
and it is per *item* and local — the same paragraph that warns against wiring
Move to `grab` says so: on a form blended from several items, magnifying one
pulls its share and leaves the rest behind. The drag has an assembled-surface
resolver and the scale does not, so `Pinçar` reaches a grid and a mesh and not
a field. Reconstructing the resolver host-side would put field math in this
application, which the layering forbids and which the engine's own note asks
callers not to do.

Every other issue filed from this work has been released and taken up —
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
| [#86](https://github.com/CyberdyneCorp/ClayCore/issues/86) whole-grid voxel meshing | 0.30.0 | **A voxel sculpt is drawn incrementally.** `clay_voxel_take_dirty_chunks` reports what an edit dirtied and `clay_voxel_mesh_chunks` meshes only those, so an edit costs the edit: 3.3 ms where meshing the grid whole cost 309 ms and rose with the sculpt. `visual_voxel_sculpt.rs` holds it as a count of chunks rather than a duration |

### Upstream: released, not yet taken up here

**Nothing.** Every issue filed from this work has been released *and* taken up.

### Upstream: available and not needed

| Issue | Released | Why it does not change anything here |
|---|---|---|
| [#63](https://github.com/CyberdyneCorp/ClayCore/issues/63) partial backend registration | 0.30.0 | A backend that loses one pipeline now registers and says what it lost, and `clay_backend_supports` / `clay_backend_diagnostic` answer why one is missing. Worth surfacing in the diagnostics report; nothing is broken without it |

The grid's triangles are *meshed* per chunk and still **assembled and
uploaded whole**: `visible_mesh_geometry` concatenates every chunk into one
buffer each time anything changes. Measured at 776,614 triangles that is 26 ms
of the 29 ms an edit costs — inside the 50 ms budget, and the same whole-buffer
shape a mesh layer has always had. The next step is a per-chunk slot layout in
that buffer, which is what `SurfaceGeometry` already does for the field side;
it is not owed until a document holds a grid past about two million triangles.

**Five numbered upstream issues are open:**
[#210](https://github.com/CyberdyneCorp/ClayCore/issues/210), an undo that
cannot say what it changed;
[#378](https://github.com/CyberdyneCorp/ClayCore/issues/378), a live brush
preview that cannot be composed with the rest of the document — which is why a
live Suavizar opens only where the layer being smoothed is the only visible
field subtool, and falls back to the held gesture otherwise;
[#321](https://github.com/CyberdyneCorp/ClayCore/issues/321), a layer with no
combine operation, which is what a live subtool boolean waits on;
[#364](https://github.com/CyberdyneCorp/ClayCore/issues/364), instance layers
with no constructor, which is what a cheap duplicate waits on; and
[#365](https://github.com/CyberdyneCorp/ClayCore/issues/365), a voxel grid
reachable only by name. Each costs latency or a cheaper implementation and none
of them blocks anything — see *What is blocked, and what is not*. Every other
issue filed from this work has been released, and
[#317](https://github.com/CyberdyneCorp/ClayCore/issues/317) is released and
now linked: the readers it promised arrived with the 0.60.0 pin and the
sidecar they retire is still here, which is a change of its own.

## What is left

**11.1, the open decisions.** Four of them, below. They gate archiving the
change and nobody but the product owner can settle them.

**11.3, archive.** After 11.1.

That is the whole list. Everything else in milestone 5 landed: masks and
armatures, document lifecycle including autosave and recovery, mesh import and
export, diagnostics, units, instrumentation, bundles and attribution, backend
parity and the cross-platform document check.

### Brush coverage, as delivered

An audit against ClayCore 0.60.0 found the brush system was not misusing the
engine — the mesh path respects the fixed-topology contract, Grab is a gesture
rather than a run of dabs, the SDF drag uses the assembled-surface resolver,
symmetry mirrors directions as well as positions — but that it did not *reach*
several verbs the pinned engine has had all along. What closed:

| Gap | What was there | What it is now |
|---|---|---|
| Voxel Paint was inert | the palette held one entry, nothing chose a colour, and the composition root wrote the vertex-colour switch off on every frame | a brush colour in the sculpt session, resolved to a palette entry and to the mesh paint stamp, and the modulation left on |
| `sculpt_grab` bound, unreachable | Mover reached a field and a mesh | Mover reaches a grid, holding the gesture and applying it once from its anchor |
| `sculpt_flatten` bound, unreachable | Planar reached a field and a mesh | Planar reaches a grid, two-sided, with the difference in the tooltip |
| `Op::Incise` reached no tool | Vinco was mesh-only | Vinco is the field's incise at 0.6 of the brush, inverting to the ridge |
| `Op::Relief` + buildup reached no tool | Argila was mesh-only | Argila is relief with buildup and a denser stroke |
| `clay_item_volume_move_topological` not bound at all | — | Mover Topológico, on fields, beside Mover rather than replacing it |
| masks kept beside the document | lost on close | attached to the layer, saved with the file, on the undo stack |

Two things the work found that the audit did not. **A drag on a grid does not
decompose**: the engine's grab resamples per cell and weights the displacement
by its falloff, so a one-cell step moves the middle of the region and not its
rim — which inside solid material is no change at all. Measured on a slab with
a 0.35 drag, delivered whole it moved material at every brush size tried, and
delivered as the eight segments a pointer makes, seven changed nothing. So it
joins the tools that land at pointer-up, at the cost of a live preview, and the
ask for one back is
[#393](https://github.com/CyberdyneCorp/ClayCore/issues/393) — which carries the
sharper measurement: the same total drag split 1 / 2 / 4 / 8 ways moves 59 / 61
/ 0 / 0 cells at a 24-cell footprint, and the coarser splits *inflate* the form
(2109 occupied cells becoming 2371) rather than translating it. And **the mask
migration was never upstream**: see *What is blocked, and what is not*.

**What it costs**, from a full `bench-compare` against the recorded Linux
baseline, on a quiet machine and with nothing flagged:

| figure | baseline | now | |
|---|---:|---:|---|
| `brush.sdf.argila.mean` | — | 4.59 | new; beside `padrao`'s 4.02 |
| `brush.sdf.vinco.mean` | — | 11.83 | new |
| `brush.sdf.movertopologico.mean` | — | 165.81 | new; it bakes, where `mover` is 390.56 and does not |
| `brush.voxel.mover.ms` | — | 36.44 | new; one-shot, beside `suavizar`'s 36.46 |
| `brush.voxel.planar.ms` | — | 36.38 | new |
| `brush.voxel.pintar.mean` | 0.78 | 0.33 | −57% |
| `brush.sdf.mascara.mean` | 0.14 | 0.23 | +69% |

The one that went up is the mask, and it went up for a reason worth paying: a
mask stroke now writes into the document's own layer and records on the undo
stack, where before it wrote into a field beside the document and recorded
nothing. Nine hundredths of a millisecond, against a fifty-millisecond dab
budget, for a mask that survives the file and undoes as one gesture.

What is still upstream, with the measurement behind each: SDF Pinçar needs an
assembled-surface resolver for the radial scale; SDF stroke alphas need the
stamp resolver to carry the template's deformer chain; `clay_item_set_gate` is
accepted and inert. A true regional field inflate and a voxel DamStandard
recipe are both decisions rather than gaps — neither should be built before
somebody has looked at what it draws.

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
| 0.39.0 | 6.3 ms | 9.5 ms | 1.5x |

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

### Where the frame time is not

The scene costs 0.56 ms a frame at 1280x800 on 296,607 triangles, against a
16.7 ms budget — 3% of it, with 4x multisampling and the occlusion passes on.
Nothing on screen is
worth optimising and everything the application does slowly is on the CPU,
meshing. That is the number to check before any rendering work is proposed.

It also means the visual budget is wide open, and one thing it will not buy is
worth recording so it is not tried twice.

**Cavity shading from screen-space normal derivatives does not work here.** A
MatCap is indexed by the view-space normal alone, so two points sharing a
normal shade identically whether one sits on an open flank or at the bottom of
a fold — which is why an unlit sculpt reads as a blob. The cheap fix for that
is to take the divergence of the normal across a pixel quad, `dpdx(n).x -
dpdy(n).y`, and darken where the surface turns into itself. It needs no second
pass and no extra target.

It produces tessellation moiré, not cavity. The reference form is 296,607
triangles over roughly 40,000 covered pixels — about seven triangles a pixel —
so the interpolated normal field is piecewise-linear at a scale *below* the
derivative's. What the derivative measures is where the triangle edges are.
Turned up far enough to see, it draws the mesh:

  target/visual/50-sculpted-surface.png, at 27x the intended strength, is a
  sphere covered in concentric rings with no cavity anywhere.

It also broke `compaction_rebuilds_the_surface_without_changing_it`, which
compares a compacted surface against the same surface before compaction: 77% of
pixels changed, because amplifying a per-triangle quantity makes the render
sensitive to which key owns which triangle.

The technique needs triangles much larger than a pixel. **What was built
instead reads depth**, which is indifferent to triangle size: positions are
shared across a triangle edge, so the depth buffer is a continuous function of
screen position however finely the surface is tessellated, and the normal the
pass needs is derived from it rather than taken from the vertex.

Two passes after the scene, and neither needs an offscreen colour target. The
first writes occlusion into a single-channel target the framebuffer owns,
sampling a hemisphere around each pixel's reconstructed view position. The
second blurs that over 4x4 and multiplies it onto the resolved colour through
the blend state — `src * dst` — so it reads no colour and no copy of the frame
exists. egui still paints onto the same target afterwards, unchanged.

  frame.median  0.48 -> 0.56 ms      (16.7 ms budget)

Measured on the reference form: 31% of pixels darker, none lighter, at most 16%
of a pixel's light. On a bare sphere the surface mean is unchanged to the digit
— nothing on a convex form occludes anything else on it, and a pass that
darkened it would be reporting its own sampling error as shape, which is
exactly what the normal-derivative version did. `visual_occlusion.rs` holds
both, against the same frame with `Renderer::set_occlusion(false)`.

It needed the multisampled depth buffer, which it bound as
`texture_depth_multisampled_2d`. A device that would not multisample the surface
format drew without occlusion rather than growing a second shader for a case no
real device reaches. **That is no longer true**, and the rest of this section is
what replaced it.

### The occlusion pass, rebuilt around what it was getting wrong

A rendering review of `main` on 2026-08-29 read the whole viewport and found
nothing to replace and a list of things to refine. The four that mattered were
all in the pass above.

**It ran at display resolution and blurred without regard for edges.** Sixteen
projected depth samples a pixel, then sixteen more texture loads in a 4×4 box —
about 33 million AO depth samples a frame at 1920×1080 and four times that at
4K. The box average is not depth-aware, so occlusion bled across silhouettes,
thin openings and disconnected pieces. That halo is what gives a screen-space
effect away.

**It was welded to multisampling** by the binding above, for a reason that is
about a texture type and not about rendering.

**Its radius was 0.08 view units**, tuned against the reference form whose
starting sphere has radius 1. An import a hundredth of that size got no visible
occlusion; one a hundred times it got total occlusion. Neither is a property of
the shape.

**It cost the same under a moving pen as it did on an idle model**, for
quality no brush decision rests on.

So it became three passes. A reduction takes the *closest covered* sample of
each 2×2 block of the multisampled depth into a single-sampled half-resolution
target — closest and not average, because an average of a foreground and a
background that met at a silhouette describes a surface that is not there. The
kernel runs on that, at a quarter of the pixels. A depth-aware upsample brings
it back, weighing each neighbour by how near its view-space depth is to the
pixel being shaded, which is what stops the average at an edge.

Measured here on a discrete card, the same reference form, occlusion at half
resolution against the same code at full:

| | half res | full res |
|---|---:|---:|
| 1080p kernel | **0.03 ms** | 0.10 ms |
| 1080p reduction | 0.07 | 0.03 |
| 1080p upsample | 0.05 | 0.05 |
| 1080p, whole chain | **0.15** | 0.18 |
| 4K kernel | **0.10 ms** | 0.37 ms |
| 4K reduction | 0.26 | 0.12 |
| 4K upsample | 0.18 | 0.18 |
| 4K, whole chain | **0.54** | 0.67 |

The kernel is three to four times cheaper, which is the arithmetic working out.
The reduction gives some of it back, and *why* is worth recording so it is not
re-investigated: at half resolution it reads sixteen multisampled depth samples
per output pixel against four at full, and it is bound by those loads rather
than by anything around them — taking its loop bounds out of the uniform and
into the shader source, so they are compile-time, changed the figure by nothing
at all.

So the performance win is about a fifth of the pass, and the real win is the
quality. `visual_ao_quality.rs` is the fixture set that says so, and each case
is a property rather than a picture:

| fixture | what it holds |
|---|---|
| `deep_crease` | a fold half as deep as it is wide still darkens |
| `thin_gap` | a gap five times deeper than it is wide is not averaged shut |
| `silhouette` | **no** background pixel more than five from an outline darkens |
| `contact` | a box on a plane still casts a shadow where they meet |
| `scale_small` / `scale_large` | the same fold at ×0.01 and ×100 shades alike |

The five in the silhouette case is arithmetic, not taste: an occlusion pixel
covers two display pixels, the upsample weighs a 3×3 neighbourhood of them, and
the block one of those was reduced from may straddle the outline — two plus two,
and one more for the multisampled edge. Beyond that no part of the pass has any
business having seen the foreground.

The scale pair is the one that took two changes to pass. Making the radius a
fraction of the form's own radius took it from 0.0% and 0.1% of the form
darkened to 2.9% and 13.8%; the depth range following the scene took it to
**2.9% and 2.9%**.

### Depth, which was spending its precision in the wrong place

`near = 0.01, far = 1000`, fixed, whatever was on screen. Two failures at once:
a thumbnail-sized import zoomed into is clipped away by a near plane larger than
the model, and a large one gets a buffer whose whole useful precision sits in
the first hundredth of the range.

The range now follows the viewing distance and the scene's radius, and it is
**reversed** — near at 1, far at 0, `GreaterEqual`, cleared to zero. Floating
point crowds its precision near zero; a conventional mapping spends that on the
far plane, where nothing needs it. The convention is stated once, in
`DEPTH_COMPARE`, and eight pipelines, a clear value, a wireframe bias and three
occlusion passes read it from there — they agreed with each other by coincidence
before that constant existed.

### What a frame is worth is decided outside the renderer

`quality.rs` holds three tiers and the hysteresis between them; the application
holds what the pointer is doing and hands the answer over. A renderer that
worked it out for itself would be a second definition of "is the user
sculpting" for the two to disagree over.

The hysteresis is the part that matters. Raising quality on every pointer
release would rebuild the frame at full cost *between two dabs of one stroke* —
which puts the cost exactly where the latency is measured. So the fall is
immediate, the settle waits 160 ms and the idle rise 600 ms, and a profile is a
ceiling rather than a target: Presentation still drops to the interactive tier
under the pen, and Performance never leaves it.

### Two bugs the profiler shipped with, and how they were found

Per-pass GPU time needs timestamp queries, and both mistakes made the *device*
stop answering — which is exactly what diagnostics must never be able to do.

**A query resolved but never written blocks the device.** The first version kept
one query set with a pair of slots per pass and resolved the whole set. A query
that was never written never becomes available, and a resolve waits for it: the
frame never completed and the driver gave up sixty seconds later. Every frame
with occlusion switched off did it — which is every frame of every capture that
compares occlusion on against off. The fix is a query set per pass, resolved
only for the passes that ran; a set can only be resolved whole, and the
destination has a 256-byte alignment, so "resolve just the pairs that ran" means
a set per pair.

**A readback mapped twice is a panic.** The second version asked for the map
once a frame for as long as a result was in flight rather than once per resolve.
An offscreen capture reads its target back and waits for the device every frame,
so nothing is ever in flight and the bug is invisible; `window_smoke`, which
presents real frames, found it on the first run. `gpu_profiling.rs` now renders
sixty frames that poll without waiting, which is the condition, and asserts that
a frame with three of the four passes skipped still completes.

### Materials, at a distance and up close

MatCaps and reference images had no mip chain, so a subtool small enough that
its normals vary by more than a texel between neighbouring pixels sampled the
material at random and sparkled as the camera moved. Both have one now. The
MatCap's levels are *rendered from the material's own recipe* at each level's
size rather than filtered down — the image is stored sRGB-encoded, and averaging
its bytes averages in the wrong space. A reference is somebody's photograph and
has no recipe, so its levels are filtered in linear colour, premultiplied by
alpha so a cut-out does not bleed its transparent texels into its edge. That is
the one place in this renderer where anisotropy earns its cost, and it is on:
a reference plane the camera has swung round to trace against is genuinely
viewed edge-on, and a MatCap never is.

Two optional terms were added beside them, both off or subtle by default and
both switched off under the pen: a **contour** that darkens toward the
silhouette, and a **cavity** that sharpens creases finer than the occlusion
radius. The cavity is the term the normal-derivative experiment above was
reaching for, arrived at from the other direction: it reads reconstructed
*positions* rather than interpolated normals, so triangle size does not enter
into it.

### Studio shading, which answers the one question a MatCap cannot

A MatCap is indexed by the view-space normal, so its lighting is welded to the
camera: orbit the form and the light orbits with it. That is exactly what makes
it good for reading form and useless for judging how a surface will take a real
light. Studio mode is a three-light rig fixed in the **world** with a filmic
curve over it, offered beside MatCap and never in place of it — `visual_studio.rs`
asserts the difference that matters, that the studio highlight travels fourteen
times as far across the form under the same orbit.

There is no HDR intermediate, deliberately. The curve is applied before the sRGB
target encodes it, which is the whole benefit of tone mapping; an HDR
intermediate buys the ability to run *post-process* effects in linear high
range, and there is no such effect here. A full-resolution `Rgba16Float` target
and a second pass to render generated grey clay would be bandwidth for nothing.

### Multisampling became a choice, and choosing it found a bug

`sample_count` wanted four samples and fell back to one; it is now an
`MsaaQuality` resolved to what the *device* will take. The default stays four
for every adapter rather than being derived from the device type, deliberately:
the obvious rule — four on a discrete card, two on an integrated one — reads the
wrong thing on Apple Silicon, which reports itself integrated and is not short
of fill rate, and would quietly have taken macOS from four samples to two.

Making it selectable at all surfaced a failure that looked like a measurement.
The resolve asked the *adapter*, which reports what the hardware has; a device
may only use the two counts WebGPU guarantees — one and four — unless it asked
for the adapter-specific ones. Choosing 2× therefore built every pipeline with a
validation error, and because a validation error here is reported rather than
fatal, the frame survived and drew nothing:

    msaa.2x.frame.median   0.03 ms      against 0.20 for none

The device now requests `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` where the
adapter has it, the resolve consults the device rather than the adapter, and the
figures read as they should:

| | 1080p |
|---|---:|
| one sample, with the post-process pass below | 0.26 ms |
| 2× | 0.24 |
| 4× | 0.32 |

The first row is worth reading twice. On this card the post-process fallback
costs *more* than two samples of real multisampling, and reads worse — it works
on the picture rather than on the geometry, so it can mistake a fine sculpted
crease for a stair-step. It is what a device gets when it has no choice, not a
cheaper setting to reach for, and the code treats it that way: it runs only
where the format refuses to multisample at all.

`multisampling.rs` now asserts the *picture* rather than the count — every
quality that resolves has to draw a form — because the count was right and the
frame was empty.

### One shadow map, in Studio mode

A key light on an unshadowed form lights the inside of every fold as brightly
as the flank beside it, which is the same failure a MatCap has. Occlusion does
not fix it: occlusion is a local term at the scale of a crease and cannot say
that an arm is between the light and a chest.

So the studio rig's key casts, through one directional shadow map — the
review's own instruction is to start with a single well-fitted map and measure
before reaching for cascades, and a form on a turntable is exactly what a single
map is for. 2048², fitted to the *subject's bounding sphere* each frame rather
than to a fixed volume: the map's resolution on the form is its side divided by
the form's diameter, so a fit that spends half its area on empty space halves
the shadow's sharpness. A sphere rather than the box, so the fit does not
breathe as the form turns under a fixed light.

Three details are worth recording because each was arrived at rather than
copied.

**The bias is along the surface normal, not along depth.** A depth bias has to
be tuned against the slope or it either lets a surface shadow itself in stripes
or lifts the contact shadow off the thing casting it. A normal offset moves the
sample to where the surface is *thicker* than a map texel, which is the quantity
the artefact is actually about — and the offset is two texels of the fitted map,
so it follows the subject's scale for free.

**The pass binds the light's matrix and deliberately not the map.** A texture
cannot be written by a pass that also holds it for reading, and wgpu refuses the
whole command buffer over it rather than the pass. So there are two bind groups
over one uniform buffer: one for the pass that fills the map, one for the shader
that samples it.

**A shadow keeps 18% of the key's light rather than reaching black.** A shadow
that reaches black is a hole in the form. The ambient and the fill are still
arriving, and a sculptor reading a shape needs the shadowed side to stay
legible.

Measured on the reference form: 2,004 of 25,052 covered pixels fall into shadow
and none come out lighter, which is a form shadowing itself rather than a rig
dimming everything. Nothing is allocated until Studio mode is first asked for —
sixteen megabytes of depth is not a MatCap session's business — and MatCap never
casts at all, because its lighting is welded to the camera and a shadow from it
would swing round the form as the view moved.

### Anti-aliasing where the device will not multisample

A device that refuses four samples on the surface format drew a stair-stepped
silhouette against a flat ground, which is the most visible thing a frame can
get wrong. It gets a post-process pass over the finished frame instead.

It runs *only* there. Four samples and a blur over the top is paying twice to
lose detail once, and what a blur loses on a sculpt is fine crease mistaken for
stair-step — the filter works on the picture rather than on the geometry, so it
cannot tell the two apart. It can also be switched off, because a filter that
softens detail is a choice rather than an improvement.

Measured against the same frame with the pass off, which is the only comparison
there is — it reads the frame's own colour, so no second render exists that
should look like it: the silhouette holds **74** pixels between the form's value
and the ground's without it and **634** with it, while the form's interior stays
flat. An outline being resolved rather than a frame being blurred.

The pass reads the scene, so the scene cannot be drawn straight into the
caller's target — a texture cannot be sampled and written by one pass. Where the
pass will run, the scene lands in a target of the framebuffer's own and the
filter writes the caller's; where it will not, nothing changes. That is decided
by what the caller *intends* rather than by what is available, so switching the
filter off costs a pass rather than adding a copy to get the frame back out.

### The brush cursor, as a ribbon

WebGPU has no line width. A line list is one pixel, always, whatever the
display — and a one-pixel line has no partial coverage, so the scene's
multisampling has nothing to resolve on it. The cursor a sculptor looks at
continuously was that line.

It is a strip of triangles now, expanded either side of the curve it stands
for. The width is decided in *pixels* and converted back into world units at
the depth each vertex sits at, so the ring reads the same weight whether the
camera is close or far — measured on a sphere at two distances, 3.5 pixels
across at one and 3.3 at twice the distance, where a ribbon expanded by a fixed
world width would have halved.

Two details are what make it a ring rather than a chain of dashes. The
expansion is perpendicular to *both* the curve and the direction to the eye,
so the ribbon faces the camera however the ring is turned — an expansion in the
ring's own plane would vanish to nothing exactly when a brush is being aimed
along a surface. And the strip is continuous, with each point's width taken
from the direction through it rather than from either segment meeting there,
so the corners of a forty-eight-sided ring close instead of leaving a notch at
every one.

The world-space semantics are untouched: the ring still shows the footprint the
brush will cover on the actual surface, which is the thing a screen-space circle
would lie about.

### The renderer, as six files

`renderer.rs` was 3,205 lines when this work started and 4,736 by the time the
occlusion rewrite, the studio rig and the quality tiers were in it. The review
asked for it to be split and, in the same breath, asked for the split not to
happen all at once: *extract functionality as it changes*. So it went out along
the seams this change had already cut.

Five of the pieces left the file entirely, as siblings:

| | lines | what it is |
|---|---:|---|
| `renderer/overlays.rs` | 923 | the grid, cursor, rig, cage, manipulator and orientation gizmo, as functions from a description to triangles |
| `renderer/pipelines.rs` | 258 | what a pipeline is, and which way depth runs |
| `renderer/shadow.rs` | 393 | the studio rig's map, and the fit that keeps its resolution on the form |
| `renderer/ao.rs` | 230 | the occlusion uniform, its bind groups, its kernel and its figures |
| `renderer/textures.rs` | 176 | the two mip chains, and why they are built differently |

and three more came out as modules of their own while the work was going on:
`quality.rs`, `profiler.rs` and `frustum.rs`.

What stayed is what needs a frame in front of it: the renderer's state, and the
pass order that state is drawn in. The occlusion *passes* are still beside the
frame they are part of even though everything they are made of moved out —
which is the line the split was drawn along, rather than "everything with `ao`
in its name".

Ten near-identical draw blocks went the same way, into one call. They differed
only in which mesh and which pipeline, and one of them setting the wrong buffer
would have drawn the wrong geometry with the right state — a picture that looks
deliberate. What is left written out is what genuinely differs: the mesh
layers, drawn span by span with a tint on the active subtool and their own
edges over them.

### Temporal occlusion, decided against

The review puts it at P2 with a condition attached — *only after the static path
is right*, and *to allow cheaper samples*. The static path is right, and the
second half of the condition is already met by other means: the quality governor
takes the sample count down to eight under the pen and back to sixteen when the
pointer stops, which is the end temporal accumulation is a means to.

What it would cost is the machinery. Two ping-pong pairs — one for occlusion,
one for the reduced depth it is validated against — a reprojection through the
previous frame's view-projection, a rejection rule, a per-frame rotation for the
history to converge over, and an application that keeps redrawing while it does.
Every one of those has a failure mode, and they share it: occlusion trailing
behind a brush. That is the one artefact a sculptor cannot work through, and it
would appear exactly where the machinery is hardest to reason about.

So it is not built, and this is the note saying the decision was made rather
than forgotten.

### What the numbers say not to build yet

The review's remaining items each carry a condition, and the conditions are not
met. Recorded here so they are not built on enthusiasm:

- **GPU-driven indirect draws** and **render bundles for the static overlays**
  reduce CPU draw submission. The reference scene submits **four** draw calls at
  1080p.
- **Packed vertices** reduce vertex bandwidth. The scene pass is 0.09 ms at
  1080p on 395,392 triangles.
- **Persistent voxel GPU chunk slots** and **vertex-only mesh patches** need the
  engine layer to say which chunks changed and to hold a stable layout across
  syncs, which `visible_mesh_geometry` does not express. The renderer side is
  ready — `patch_vertices` and `patch_indices` have been there since the SDF
  path was written, and buffers now grow geometrically so a patch does not
  reallocate — but throwing the switch means guessing whether topology changed,
  and a wrong guess is a stale index buffer, which is a wrong picture. What was
  taken instead is the safe half: the polyframe's edge set, a hash set over
  three entries per triangle, is no longer derived on every mesh upload when the
  polyframe is off, which it is by default.

Per-subtool frustum culling *was* built, because its condition is different: it
costs six plane tests against a box per span, it is bounded by the number of
subtools rather than by anything that scales with the frame, and a scene of
fifty subtools is a thing a sculptor can make today.

### Undo, which cost far more than the edit it takes back

**Taken.** `clay_document_undo_bound` / `_redo_bound` report the world box of
what a step applied, and the application now re-meshes that instead of the
active layer's whole bound. Measured on the same fixture, moments apart —
1045 surface bricks after 96 edits:

| | keys | engine | sync |
|---|---:|---:|---:|
| a dab | 18 | 0.76 ms | 7.49 ms |
| undoing it, before | 1045 | 24.36 ms | 273.69 ms |
| undoing it, after | 18 | 0.94 ms | **8.63 ms** |

It also fixed a bug rather than only a cost: the old fallback was the *active
layer's* bound, so undoing an edit made on a different subtool re-meshed the
wrong one and left the changed surface stale — the undo looked like it had done
nothing. The section below is what that was.

### Undo, which cost far more than the edit it takes back — as it was

Nothing measured undo until it was asked about, and it was 367 ms to reverse a
dab that cost 5 ms. On 1043 surface bricks after 96 edits:

| | keys meshed | the edit | the sync |
|---|---|---|---|
| a dab | 27 → **18** | 0.9 ms | 4.3 → **3.6 ms** |
| undoing that dab | 2940 → **1045** | 83 → **66 ms** | 284 → **141 ms** |

Taking up ClayCore 0.39.0 halved both columns again without a line changing
here — an undo is 13.7 ms in the engine and 68.2 ms in the re-mesh, and the dab
it reverses is 0.3 ms and 1.9 ms. The ratio is what the issue below is about
and it has not moved: an undo still costs about forty times the edit.

Two things, one ours and one not.

Ours: a dirty set is an edit's *influence bound*, which is a box, and a box
around a surface is mostly not surface. A third of a dab's keys and two thirds
of an undo's were uniformly inside or outside — no lattice, no triangle
possible — and were being marched anyway. `sync` now asks
`clay_brick_cache_read_bricks` for the state of the keys it was handed and
meshes only the ones holding a lattice, while still *replacing* all of them, so
a brick the surface has left still loses its triangles. Asked per key rather
than by intersecting with `surface_bricks`, which would make a dab pay for a
copy of the whole cache to learn about nine keys.

That also removed a real inversion: the incremental path had been costing
nearly twice a full rebuild of the same surface (284 ms against 152 ms), so the
fast path was the slow one. It is now within noise of a rebuild, which is the
right place for it — a dirty set can never exceed the surface, so past this
point there is nothing a rebuild-instead-of-patch crossover would buy. It was
measured and left out rather than added on principle.

Not ours: `ClayDocument::undo` bounds its refill by the whole layer, because
`clay_document_undo` reports only *whether* it undid something.
`mark_dirty_layer` is what the header calls "what a first, full fill marks", so
every undo pays a full fill. The host cannot narrow it — diffing the layer's
nodes catches adds and removes but silently misses an undone move or resize,
which keeps its id, and under-dirtying leaves stale bricks on screen. Filed as
[#210](https://github.com/CyberdyneCorp/ClayCore/issues/210): an undo that
reports its influence bound would take this to about the cost of the edit.

`undo_cost.rs` holds the half that is ours, in keys rather than milliseconds —
a sync may never mesh more keys than the surface has.

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

### Subtools: what switching costs, and what a boolean costs

Six new figures, all on the Linux baseline, all measured on the reference
suite:

| figure | cost | what it is |
|---|---|---|
| `subtool.activate.sdf` | **0.00 ms** | making a field subtool the sculpt target |
| `subtool.activate.mesh` | **0.00 ms** | making a *carried mesh* subtool the sculpt target, once its mesh has been welded |
| `subtool.solo` | 14 ms mean, 21 ms p95 | showing one subtool alone and putting the scene back |
| `subtool.solo_undo` | 203 ms | a ⌘Z after a released solo, against the 87 ms an undo costs alone |
| `subtool.copy` | 4.3 s | one subtool sampled into a subtool of its own |
| `subtool.boolean` | 10.2 s | two operands sampled and combined into a third subtool |

**Activation on a mesh subtool was the one figure that missed a stated bound,
and it no longer does.** The specification says no engine operation may block
the interface thread for more than 16 ms; this was 160, all of it
`MeshSculptor::for_layer` — a weld and an adjacency pass over the layer's
296,216 triangles — paid whenever a mesh layer became the one being worked on.

What made it a *repeated* cost was that the document held one sculptor. A
second carried mesh evicted the first, so going back and forth between two mesh
subtools — the arrangement a scene of subtools invites, and the one the figure
measures on purpose — paid the pass on every switch. The document holds several
now, bounded and least recently used first
(`clayspace-engine/src/sculptors.rs`), and a switch onto a mesh already welded
once is a lookup: **0.00 ms against the 16 ms bound**.

Holding more than one buys back that cost and introduces one bug class in
exchange — a sculptor outliving the mesh it was built over. The engine refuses
rather than reads freed storage, since its handle "remembers what it was built
over and every call checks that the answer has not changed", so the failure
this side would see is a brush that quietly stops working rather than a crash.
The sculptor for a layer is dropped when the layer is removed, and when
reconciliation finds one that left the scene or came back into it;
`clayspace-engine/tests/mesh_sculptor_cache.rs` holds the outcomes.

**What is left is the first weld of a given mesh**, once per mesh per session —
about 165 ms, and after reopening a document it is paid on the first click on
each mesh subtool. That one is inherent rather than unfixed. It cannot move to
the first dab: with no sculptor a mesh layer answers no pick, the interface
sends no stroke where the pick reported nothing, so the first dab never
arrives — measured, and held by `the_pointer_finds_an_imported_mesh` and
`the_mesh_reports_what_its_queries_cost` in
`clayspace-engine/tests/mesh_sculpting.rs`, which fail the moment the arming
comes out. It cannot move to document open either: opening the two-mesh
document above costs 44.8 ms, and warming its sculptors there would make it
about 395 ms — a nine-fold regression on open, paid whether or not the sculptor
is ever used, to save a one-time cost on a click.

Where it could go is a worker thread, and that is an engine question rather
than a host one: `clay_mesh_sculptor_create` resolves its mesh through a
mutable path into the document, and the ABI's only threading contract is the
brick cache's. Asked upstream as
[#368](https://github.com/CyberdyneCorp/ClayCore/issues/368), which quotes that
contract as the shape the answer could take.

**A boolean is two of a copy, and a copy is one sampling.** 4.3 s for one
operand at the brick cache's 0.02 cell over the reference form's box; 10.2 s for
two of them over the pair's larger box plus the layer that holds them. Neither
runs unasked: the panel states the cost in cells and in what the resolution
loses, and nothing happens until it is confirmed — which is the whole reason the
figure is allowed to be seconds rather than milliseconds. The resolution is the
sculptor's, and it is the term these figures scale with.

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

The performance gate compares against the baseline for the platform it runs on
— `benchmarks/baseline-macos-aarch64.json` or
`benchmarks/baseline-linux-x86_64.json` — and fails on a regression. One per
platform because comparing a Linux run against a macOS recording measures the
difference between two machines and calls it a regression: `just bench-compare`
picks by `os()`, and each file's `conditions` block says which machine, backend
and engine produced it. Budget breaches are printed but not enforced without
`--enforce-budgets`: the specification gates on a change *raising* latency, and
a gate that is red the day it is installed is one people learn to ignore.

The Linux baseline reads engine 0.52.2 and the pin is at 0.60.0. It was left
there deliberately when the pin moved: everything that moved across that
upgrade moved *downward* — a dab's p95 1.88x, solo's p95 1.61x, the locality
dab 1.40x, undo 1.13x, over two full runs — and re-recording would spend a
baseline whose only purpose is to catch the next thing that goes up. Two runs
rather than one because the first reported a 53% regression on a live boolean
drag that the second put back inside the spread; the machine was shared during
the first. A filtered `bench-only` run is not evidence either way: asked for
the dab group alone it reports a median of 5.56 ms where the full run reports
1.62, which is the reason the recipe refuses to record a baseline from one.

The figures below are that baseline's own conditions: engine 0.52.2,
CUDA, 1280×800, at 0.13 load per core, with a dab median of 2.10 ms against a
50 ms budget and a locality key ratio of 0.75 against a budget of 2. The macOS
baseline still reads engine 0.29.1 and cannot be re-recorded from here — it
takes a run on that hardware, the same condition the macOS CI rows are waiting
on.

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

**A mask can only be written one call at a time, and every call costs the whole
mask.** A document-owned mask records for the undo history by snapshotting its
entire chunk map on the first write inside a call and diffing it when the call
returns (`clay_c.cpp`'s `MaskStep`, `voxel::MaskField::touch`). Every entry
point brackets its own step — `clay_mask_set`, `clay_mask_paint_cell`,
`clay_mask_fill`, `clay_mask_invert_within` — and there is no entry point that
takes a *set* of cells, merges a second mask, or lets a mask gate a brush
(`clay_mask_paint` documents that `brush->mask` is ignored: "a mask does not
gate itself"). Measured on a mask covering a million cells:

| writing a region | calls | time |
|---|---|---|
| one-cell fills, document mask | 5000 | 21.2 s |
| one-cell fills, standalone mask | 5000 | 35 ms |
| the region as one stamp run | 1 | 163 ms |

The standalone row is the same work with the history switched off, so the cost
is the snapshot rather than the write. The drawn mask gestures are built around this: the
region is delivered as a **path that visits it**, walked once by
`clay_mask_apply_stroke`, which is the only entry point that writes many cells
for one snapshot.

What that costs is a covering factor. The path's lattice is aligned to the
camera and a brush footprint is aligned to the world, so the footprint has to
reach half the lattice pitch's diagonal rather than half its side, and every
cell of the region is written about 2.7 times — 659 ms for an outline thrown
around the whole of the reference form, against a floor of about 200 ms if the
footprint tiled. Opening the pitch does not help: it divides the stamps by the
cube and multiplies each one's footprint by the same, so it buys the region's
edge and nothing else. A region larger than the ceiling is refused rather than
coarsened.

A bulk cell write, a mask step a host could hold open across several calls, or a
footprint the caller could orient would each close this; none exists in 0.60.0.
Worth filing upstream, and not blocking: see
[features.md](features.md#freezing-a-region-by-drawing-round-it).

`mask.outline` measures the extreme — an outline around the whole of the reference
form, 659 ms at 0.21 load per core — and is **not** in the Linux baseline.
That file was recorded against ClayCore 0.52.2, and `bench-compare` on this tree
is already red without any of this work: run on a clean checkout of `main` it
reports regressions across the SDF brushes, the crossings, locality and startup,
which is the engine pin moving rather than anything a change did. The figure
goes in when the baseline is re-recorded for 0.60.0. A figure the baseline lacks
is reported as `new` and does not fail the gate, so nothing is hidden by leaving
it out.

**A Move drag costs the field one grab, and a session of them still
compounds.** A drag warps every item it reaches with a `grab` deformer, and the
engine's Lipschitz bound for a chain is the *product* of its links — so the
safe step scale decays by a constant factor per grab and the marcher pays for
it. Through 0.60.0 the application wrote one grab per *segment*, so a drag cost
the field as much as it was finely cut: measured on the starting form, twelve
drags of six segments each left a chain of 72 and a step scale of 0.000608,
and a dab went from 5.2 ms to 26 ms. `clay_sdf_move_*` fixed the segment half —
the same twelve drags now leave a chain of 12 and a step scale of 0.002456,
which is one grab per gesture and the factor is exactly the segments per drag.
Driven through the ViewModel, as a sculptor drives it, a twelve-drag session on
the starting form:

| drag | per segment, as it was | per gesture |
|---|---|---|
| 1st | 19.6 ms | **7.3 ms** |
| 6th | 109.1 ms | **22.8 ms** |
| 12th | 218.7 ms | **59.5 ms** |

What is left is the *gesture* half, and it is the engine's rather than ours:
twelve drags is still twelve links, and 0.606 per link is still geometric. The
escape route the engine offers is `clay_sculpt_policy`'s `max_deformer_chain`
plus `allow_consolidation`, which collapses the layer inside the stroke's own
undo step. It is not taken, and the measurement is why. Six gestures of six
segments on the starting form, then a collapse, then the same gesture again:

| brush | before the collapse | after | |
|---|---|---|---|
| Polir | 2647 ms | **202 ms** | 13x better |
| Suavizar | 161 + 211 + 223 ms | 172 + 232 + **543** ms | 1.5x worse |
| Mover | 76 + 135 ms | **754 + 591** ms | 6x worse |

Consolidation cures the mechanism it was designed for — a chain of stacked
baked volumes, which is Polir and Planar — and makes both *live* brushes worse,
because a collapsed layer is one 3.3 MB dense volume and every verb that
re-samples or warps it now pays per sample what it used to pay per primitive.
Move's step scale actually *improves* over the collapse, 0.00275 to 0.08090, and
the gesture is still six times slower: the marching win is swamped by the cost
of evaluating warped samples.

Until that is answered upstream, a Move-heavy session's escape route is the
manual Optimize on the layers that are *not* being dragged, and Optimize stays
a sculptor's decision rather than something a stroke does on its own.

**The chain bound ignores that a grab has finite support.** `CLAY_DEFORM_GRAB`
is "identity past r", so two grabs whose balls do not overlap cannot compound
anywhere — but `deformer_lipschitz` multiplies them regardless. Measured on a
radius-4 sphere, eight drags of radius 0.3 with their centres 3.06 apart report
a Lipschitz of **354.871**, which is exactly what eight drags piled on one spot
report. A sculptor working all over a model therefore pays a bound describing a
compounding that cannot physically happen, and that — rather than consolidation
— is the cheap fix.

**A region bake can only append or collapse the whole layer.** Suavizar, Polir
and Planar sample a region into a volume and put it back, and the only way to
put it back is `clay_layer_add_item` — so the layer grows one baked volume per
gesture and every later bake samples all of them. The engine's own words:
*"a polish samples a document and hands back a volume, so the SECOND pass
samples a volume rather than a document."* Twelve gestures on one patch of the
starting form:

| brush | 1st gesture | 12th | items |
|---|---|---|---|
| Polir | 22 ms | **244 ms** | 2 → 13 |
| Suavizar | 458 ms | **939 ms** | 2 → 13 |

Split by phase, Suavizar says exactly where the cost is: the transaction's own
dabs are **flat** across the session, 197 ms to 223 ms, and both phases that
touch the document grow — `_begin`'s whole-layer sample 110 to 205 ms, and the
bake that installs the result 151 to 512 ms. The transaction insulates the dabs
from the accumulation and cannot insulate its own endpoints.

The other option the ABI offers is `clay_sdf_smooth_commit`, which installs the
volume as the layer's *one* item — consolidating the whole subtool on every
stroke, and measurably worse on Metal (ClayCore#379). What is missing is the
middle: merge a baked region into the layer and leave the parametric items
outside it alone, which would make repeated work on one patch O(1) in gestures
rather than O(n). Filed upstream.

**The Move transaction's preview is not carried by the C ABI.** ClayCore's C++
`SdfMoveTransaction` exposes `preview_layer()` — a private copy of the layer
with the affected chains replaced, which the sdf-sculpt-transaction spec names
as the way a host draws a Move preview, "so it compiles, draws and picks like
any other layer". `clay.h` exposes the resolved grabs and not the layer, and
this application can only reach the C ABI. So the drag is drawn the other way
the header invites: the grabs are written onto the layer, sampled into the
brick cache, and undone inside the same segment. It works, and
`live_transactions::a_preview_grab_can_be_drawn_and_taken_back_under_an_open_drag`
holds the three facts it rests on — a written grab moves the surface, each is
one undo entry, and a commit accepts a layer that was edited and restored,
because its stamp is derived from content. But it spends two document edits per
pointer event to draw something the engine already has in hand, and it would be
a plain `preview_layer()` read if the ABI carried one. Worth asking upstream for
alongside a remove-deformer call, which the ABI also lacks: `add` has no
inverse but undo.

**The live Smooth's commit is not used, and that is a decision to revisit.**
`clay_sdf_smooth_commit` installs the working volume as the layer's one item,
so every stroke would consolidate the whole subtool. It measures worse on
Metal than on CPU or Vulkan — 7.82 roughness against a ceiling of 6.00, where
the same stroke leaves 5.74 here, with Planar and Polir identical across both
platforms as the control (ClayCore#379) — and it is heavy everywhere, since it
discards the edit list and re-samples the subtool at the cache's cell size. So
the transaction draws the preview and the stroke is laid down by the bake that
was always used. The cost is that the two are different computations of the
same smoothing; measured, they land 0.09 apart in roughness. If #379 is
answered, taking the commit would make the preview exact and is worth
re-measuring then.

**A live smoothing gesture costs 186 ms when the pointer goes down.** That is
the transaction sampling the whole layer once, and it is the trade the design
makes rather than an inefficiency: it is what makes every dab afterwards cost
what it touches (~5 ms) instead of re-baking the layer. The escape route, if it
is ever felt as a stall on a heavier model, is to sample coarser than the brick
cache draws at — which would cost resolution at the commit, so it is not taken
while the number sits where it does. `clayspace-engine`'s `live` module holds
the measurements.

**Move still leaves one grab per segment.** Measured, a ten-segment drag leaves
a deformer chain of ten where `clay_sdf_move_*` would leave one, and a long
session's chains are what the complexity policy eventually collapses. The
transaction is wrapped and tested (`claycore/tests/live_transactions.rs`) and
deliberately not adopted: it writes nothing to the document until it commits
and, unlike Smooth, hands over no samples a host can draw — only grab
parameters to reproduce. Taking it would cost Move the live picture it already
has, which is the wrong trade. It becomes takeable if ClayCore grows a preview
for Move that a host can draw, or a forward point warp for a grab.

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

