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
