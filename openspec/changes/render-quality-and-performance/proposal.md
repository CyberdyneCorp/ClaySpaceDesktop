# Refine the renderer: cheaper occlusion, honest depth, a presentation mode

## Why

The viewport already has the right architecture for a sculpting application —
MatCap over a forward pass, 4× MSAA, `Depth32Float`, SSAO derived from the
scene's own depth, incremental GPU patches, no G-buffer. A rendering review of
`main` (2026-08-29) found nothing to replace and a list of things to refine.
This change carries out that list.

The costs it names, measured against the code as it stands:

**Occlusion is paid at full resolution and then blurred without regard for
edges.** `ao.wgsl` runs sixteen projected depth samples per viewport pixel and
the composite loads sixteen more texels in a 4×4 box. At 1920×1080 that is
about 33 million AO depth samples a frame, and at 4K four times that. The box
average is not depth-aware, so occlusion bleeds across silhouettes, thin
openings and disconnected pieces — the halo that gives cheap occlusion away.

**Occlusion costs the same whether the pen is moving or the model is idle.**
There is no quality state at all, so the frame under the brush pays for
quality that no brush decision depends on.

**Occlusion is coupled to multisampling for no architectural reason.** The
pass binds `texture_depth_multisampled_2d` and loads sample zero, so
`Framebuffer` allocates the occlusion target only when `samples > 1` and a
device that will not multisample renders with no occlusion at all.

**Depth precision is a fixed `near = 0.01`, `far = 1000.0` under a
conventional depth mapping**, whatever the model's scale, so a close zoom on
a small form and a large imported mesh get the same and neither gets the
right one.

**The opaque surface pipeline blends.** `make_pipeline_with_depth` sets
`ALPHA_BLENDING` on every target including the solid one, and ties depth
writing to the `cull` flag — one boolean carrying two unrelated decisions,
which is exactly the kind of coupling reversed-Z and transparent helpers will
break on.

**Nothing measures the GPU.** The project benchmarks the sculpting path
carefully and the rendering path not at all; CPU time around
`begin_render_pass` measures submission, not execution.

**AO radius and bias are absolute constants** tuned against a reference form
of radius 1, so an imported model at a hundredth or a hundred times that scale
gets occlusion that is either invisible or total.

## What Changes

Eight phases, in the order the review recommends, each landing on its own.

- **Baseline.** Deterministic offscreen render benchmarks and AO capture
  fixtures recorded *before* anything moves, so every later claim is measured
  rather than asserted.
- **Cleanup.** Pipeline state becomes an explicit struct — cull mode, depth
  write, depth compare, blend, each stated — the opaque path stops blending,
  AO bind groups are built with the framebuffer instead of per frame, and a
  GPU timestamp profiler reports per-pass time where the adapter supports it.
- **Half-resolution occlusion.** A depth reduction pass takes the closest
  covered sample of each 2×2 into a single-sampled half-resolution depth
  target; AO runs there; a depth-aware bilateral upsample replaces the box
  blur. AO stops depending on MSAA. Radius and bias become fractions of the
  scene's own radius.
- **Occlusion optimisation.** A precomputed kernel, an integer hash in place
  of the `sin` rotation, an interaction-aware sample count, and temporal
  accumulation with history rejection — added last, and only once the static
  path is right.
- **Material quality.** Mipmapped MatCaps generated per level from the
  procedural recipe rather than gamma-downsampled, mipmapped reference images
  with anisotropy, an optional rim term and an optional screen-space cavity.
- **Depth.** A dynamic near/far range derived from camera distance and scene
  bounds with hysteresis, then reversed-Z as an isolated step.
- **Scale.** Per-subtool bounds and frustum culling, geometric buffer growth,
  vertex-only mesh patches that do not re-upload indices, persistent voxel
  chunk slots, and render bundles for the static overlays.
- **Presentation.** A Studio shading mode — key/fill/rim, HDR target, ACES
  tone mapping, optional environment lighting and one directional shadow map —
  offered beside MatCap and never in place of it, plus weighted-blended OIT
  for the transparent helpers and FXAA where MSAA is off.

Three things are held to explicitly. MatCap stays the default sculpt mode.
Every quality addition is switched off, or dropped to an interactive tier,
while a stroke is in progress. Nothing in this change moves field math into a
shader.

## Impact

- Affected specs: `viewport-rendering`, `performance-budgets`
- Affected code: `crates/clayspace-view/src/` — `renderer.rs` splits into a
  `render/` module, `gpu.rs`, `camera.rs`, `matcap.rs`, `offscreen.rs`, the
  WGSL sources, and new `quality.rs`, `profiler.rs`, `frustum.rs`
- Affected code: `crates/clayspace-app/src/` — geometry upload paths, the
  frame loop that will now state its interaction quality, and the diagnostics
  view
- New test targets: render benchmarks and AO/depth/material capture fixtures
