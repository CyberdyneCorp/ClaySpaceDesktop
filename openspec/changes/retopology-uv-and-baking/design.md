# Design

## The seam, and why it is a format rather than a link

CyberRemesher states it as a rule about itself: *"there is no build or link
dependency on any sculpting or volumetric engine, and there never will be — the
format and the evaluator interface are the only coupling points."* ClayCore's
header states the mirror image. Neither engine knows the other's types.

This application is the only place both are present, so it is the only place the
correspondence can be made, and it is made **twice, in two directions**:

| direction | mechanism | why this one |
|---|---|---|
| sculpt out | `clay_mesh_save_handoff` → their reader | a file or a buffer; version-gated at 1.0 by both sides |
| field in | our callbacks → `CyberFieldEvaluator` | no mesh at all; the bake marches the actual surface |

The second is the interesting one. Their `cyber_bake_field` takes three C
function pointers and a `void*`, and ClayCore answers all three. A bake then
samples the *field* rather than a tessellation of it — no cage-ray misses on a
triangulation, and normals from exact gradients.

## Three traps that are already written down

None of these are discoverable from our side by reading our own code, and all
three produce output that looks plausible:

1. **Their `occlusion` is OPENNESS.** 1 is fully open. `CLAY_MEASURE_OCCLUSION`
   is occlusion — 1 is fully enclosed. ClayCore's header states the correspondence
   as `1.0f - clay_measure_points(...)` and says passing ours straight through
   "bakes an inverted ambient occlusion map, which looks plausible and is wrong
   everywhere."
2. **Curvature is not curvature.** `CLAY_MEASURE_CURVATURE` is a saturated
   `[0,1]` masking value; theirs is signed mean curvature in `1/length`. The
   instruction is to *leave theirs to its default*, which derives it from
   `gradient()`, rather than substituting ours.
3. **The handoff refuses our best export.** `clay_mesh_save` declares a mesh's
   quads as its faces when it has them, and their reader rejects any arity but
   triangles. `clay_mesh_save_handoff` exists precisely because of this: it
   always triangulates and always computes normals. **Use the handoff writer,
   never the general one.**

Each becomes a test rather than a comment, because a comment describing an
inversion is exactly what a later reader "simplifies".

## Where the crates sit

The `claycore-sys` / `claycore` pair is the pattern, and it is followed rather
than varied:

```
vendor/CyberRemesherAndUV        submodule, pinned to the v0.7.0 tag
crates/cyberremesh-sys           CMake build + bindgen; unsafe allowed
crates/cyberremesh               safe wrapper; the only other unsafe
crates/clayspace-model           RetopoModel / UvModel / BakeModel traits, domain types
crates/clayspace-engine          implements them over cyberremesh + claycore
crates/clayspace-vm              a view model per capability, over jobs
crates/clayspace-view            panels
```

`tools/check_layering.py` gains the same forbidden edges the engine already has:
the View, the ViewModels and the agent crate may not reach `cyberremesh` or
`cyberremesh-sys`, and the two new crates join the `unsafe` allowlist. A rule
that is not in that file is a rule nobody enforces.

## Performance: what is decided, and what is measured

**Decided.** The `cpu-headless` preset, a bounded worker pool, every call on the
jobs thread, and a refusal during a gesture. Each is a line of code and a
requirement, not a hope.

**Measured, and this is the part that can fail.** The claim "sculpting does not
slow down" is not observable from a retopology benchmark; it is observable from
the figures that already exist. So:

- `dab.*`, `brush.*`, `locality.*` and `tape.*` are recorded before the library
  is linked and again after, **on the same box in one sitting**, and compared as
  a ratio. A ratio transfers where an absolute does not.
- The comparison is against a **fresh** recording rather than the committed
  baseline, which was taken at engine 0.52.2 and cannot separate this change
  from thirty-two engine minors.
- A new `retopo.*` group measures the operations themselves, so that they have a
  history from their first day rather than from the first time someone
  complains.

The honest risk this cannot rule out: a second 7.3 MB shared object with its own
`libgomp`/`libtbb` runtime in the process may cost something at load or in
memory that no per-operation figure shows. `startup.*` and `memory.*` already
exist and are read for exactly that.

## Two questions this change does not settle

- **Which quad method by default.** Their own README records that solver routing
  was measured wrong until 2026-08-24, and that choosing per input by measuring
  both solvers "is still open work". We take their default and do not invent a
  routing rule of our own.
- **Whether a retopologised mesh becomes a subtool or replaces one.** It arrives
  as a new subtool here, because a retopology a sculptor cannot compare against
  the sculpt is one they cannot judge. Replacement can be added later; the
  reverse cannot be undone.

## Guided retopology session (#212)

Flow guides and density samples are authored at picked world positions and kept as
retopology session data, independent of brush settings and ClayCore geometry.
The session stores each guide's control points, mode, strength and influence
radius, plus density samples. A versioned companion file beside the
document keeps that data on save and reopen. Empty or invalid entries are
rejected on read rather than sent to the remesher.

The host meshes only the selected subtool for a run. Mesh, Dynamic and Multires
already expose their current triangles; Voxel uses its grid mesher; SDF uses a
temporary isolated mesh with other visible SDF layers restored immediately.
No conversion layer is published and no sculpt stroke is recorded. The model
maps world guidance into the selected layer's local coordinates, then hands
owned geometry and guidance to the worker. The adapter samples density
onto that geometry's vertices and passes mode-bearing guides through the
remesher's guided C ABI. Its warnings reach the job notice.

The retopology interaction mode captures pointer gestures before the sculpt
dispatcher. Guides and density have separate overlays and their edits remain
visible before a run.

## Holding the result for a decision (#211 task 11.1, #213)

**The job's result is held, not placed.** When a run lands, the ViewModel
checks the source revision as it did before publishing, and keeps the
`RetopoResult` with the settings it was asked with. Nothing reaches the
document: no layer, no history entry, no modified mark. Accept calls the same
`place_retopology` the job used to call on landing, with the same settings, so
an accepted preview is the old publish bit for bit — held by
`an_accepted_preview_is_what_publishing_placed`, which compares both paths'
layers and the preview drawn before acceptance. Discard calls
`discard_retopology`, which forgets the recorded target so the result cannot be
placed afterwards, and changes nothing else — the saved document is byte-equal
before and after.

**Where a held preview goes.** It is not document data, so its life is the
ViewModel's:

- *A new run* drops it before reading its source: the document records one
  retopology target at a time, and the new run is the sculptor's answer to the
  preview in front of them.
- *A source that moves* — a stroke, an undo, a redo, anything that moves the
  revision the result was checked against — drops it on the next frame with a
  notice, and an accept that races the frame is refused by the same check. The
  rule is the one 11.2 applies to a result landing on a moved source.
- *A save* leaves it held and writes none of it; the source revision does not
  move, so it stays acceptable.
- *A new, opened or reverted document* drops it silently: it was never part of
  the document, so there is no work to warn about, and the unsaved-work guard
  is not consulted for it.

**It is drawn in place of its source.** The engine places the result's
triangles by the source's transform through the arithmetic `uv_preview` uses
for an accepted layer (`ClayDocument::retopo_preview`), with the result's own
normals or area-weighted ones. The renderer draws it from buffers of its own
through the surface pipeline, skips the source's span and its polyframe lines
(`polyframe::Lines::spans`), and draws the result's authored edges when the
polyframe is on, through `polyframe::lines` as an accepted layer's are. A
field source is not a span of the carried buffer and cannot be cut out of the
one field surface, so the field surface is left out while its preview is held:
drawn over it, the preview lost the depth test almost everywhere, because its
quads chord the isosurface and lie just inside it. That hides any other field
layer for as long as the preview is held, which is the price of seeing the
quads at all. The preview is the result drawn alone: an in-place accept leaves
exactly that on screen, and a beside accept adds the source back next to it, as
placing beside always has. While a
preview is held, the UV display is about the preview: `UvViewModel::hold_preview`
decides whether there is a layout to show from the held result rather than the
active layer.

**The UV square is a picture of the layout.** `UvLayout` carries the
per-vertex UVs and triangles with the islands and seams `uv_islands` already
finds, now with every side of each seam (`seam_sides`) since both sides are
different edges in UV space. The panel draws it as one egui mesh — islands
tinted as on the surface, or one neutral fill under the plain checker — with
the seams in the seam colour. It is shown while a UV display is chosen and the
active layer or held preview carries UVs, and pushes no command.

## From fixed mesh to hierarchy (#214)

**Create Multires is one action, priced as a whole.** A crossing followed by a
Subdivide click per level priced each level only when it was asked for, so the
cost of the hierarchy a sculptor meant to make was never stated before the
first allocation. `HierarchySettings { levels, in_place }` asks for the whole
thing and `HierarchyPlan` prices it before anything is built: the document's
ledger and the cage (built as a hierarchy of one level, weighed and dropped)
are measured, level 1 is the engine's preflight of that cage, and every deeper
level is projected at four times the one below — the engine prices a level
from the one below it, and the one below does not exist yet. The build then
prices every level again with the engine as it goes, on top of the ledger and
what the free-standing hierarchy holds, and the layer is made only once all of
it stands; so a refusal at any point leaves nothing behind, and a success is
one crossing and one undo.

**A level is charged at what it holds once drawn.** Measured on a 16×16 quad
cage at level 4, the engine's `peak_bytes` quoted 3,741,720 bytes while adding
and drawing the level grew the document by 20,886,292. The preflight prices the
build alone, and this application draws the level it has just built. The same
preflight quotes the evaluated surface and the runtime index as held while a
level is resident; `SubdivisionCost::resident_bytes` is persistent + evaluated
+ runtime (40,623,342 there), and a level is charged at the higher of that and
the peak. That holds for a Subdivide click as much as for Create Multires.

**The plan charges every level drawn, and so errs high.** Charging only the top
level drawn left the build's own per-level check able to refuse a plan that had
been shown to fit: the levels below carry a runtime index the preflight does not
itemise. Charging each level at its resident figure makes the plan a bound the
build is held to (`a_plan_that_fits_is_built_and_holds_no_more_than_it_quoted`,
at 8², 16² and 32² cages), at the price of quoting about twice what a drawn
hierarchy measured: 55,127,094 bytes quoted against 27,879,079 held at four
levels over the 16×16 cage.

**The projection is measured, not trusted.** Four times each figure was never
below the engine's own quote of the next level and at most 7.7% over it across
levels 1–4 on the three cages (`the_projection_never_undercuts_the_engine`).

**Kept between frames.** The plan walks the ledger and copies the cage, so the
scene ViewModel keeps it keyed by the active layer, the history depth and the
layer count, and asks again when one moves.

**What the engine could give instead.** An exact figure for a level as it is
held once drawn — or a preflight whose peak includes the evaluation a host
needs to draw it — would replace the resident upper bound and let the plan
charge levels below the top at what they actually hold. Nothing in this
application depends on that; it would only make the quote tighter.
