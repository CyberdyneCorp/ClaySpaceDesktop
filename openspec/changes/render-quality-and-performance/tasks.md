# Tasks

## 1. A baseline to measure against

- [x] 1.1 Add deterministic offscreen render benchmarks — a scene set and a
      viewport-size set — reporting GPU frame, scene pass, AO, composite,
      draw calls and bytes uploaded
- [x] 1.2 Record the numbers the occlusion change has to be argued against —
      taken as an A/B of the same code at full and half resolution rather than
      against a stale baseline, since the two differ in one constant
- [x] 1.3 Add AO capture fixtures for the cases a cheaper AO can break: a deep
      crease, a thin gap, a silhouette, a contact shadow, and the same form at
      a hundredth and a hundred times its scale

## 2. Say what a pipeline is, once

- [x] 2.1 Replace the `cull: bool` that silently also decides depth writing
      with a state struct naming cull mode, depth write, depth compare, blend
      and bias separately
- [x] 2.2 Stop blending on the opaque surface path; keep blending where it is
      meant — ghost, membrane, reference, wire
- [x] 2.3 Build the AO and composite bind groups with the framebuffer rather
      than once per frame, and rebuild them when it is recreated
- [x] 2.4 Hold the uniform layouts to their WGSL sizes in tests, so a struct
      that grows on one side of the boundary fails here rather than on a device

## 3. Measure the GPU

- [x] 3.1 Add a timestamp-query profiler over the named passes, reporting
      `Unsupported` and rendering normally where the adapter has no timestamps
- [x] 3.2 Surface per-pass GPU time, AO resolution and sample count, draw
      calls and upload bytes in the diagnostics view

## 4. Occlusion at half resolution

- [x] 4.1 Add a depth reduction pass: multisampled full-resolution depth in,
      single-sampled half-resolution depth out, taking the *closest* covered
      sample of each 2×2 rather than an average, which would invent a surface
      between a foreground and a background that met there
- [x] 4.2 Allocate the occlusion target at half resolution and carry its own
      extent in the AO uniform, distinct from the scene viewport
- [x] 4.3 Bind the reduced depth instead of the scene depth, so occlusion no
      longer depends on the device multisampling — and allocate the occlusion
      target whatever the sample count is
- [x] 4.4 Replace the 4×4 box composite with a depth-aware bilateral upsample
      weighted by screen distance and depth similarity
- [x] 4.5 Make radius and bias fractions of the scene's radius rather than
      absolutes, so a model at any scale gets the same occlusion

## 5. Occlusion for less

- [x] 5.1 Precompute the sample kernel into a uniform, leaving the per-pixel
      work a rotation rather than a `sqrt`, `cos`, `sin` and `mix` per sample —
      and record that it bought nothing measurable, because the pass is bound
      by its texture fetches rather than by arithmetic
- [x] 5.2 Replace the `sin`-based rotation hash with an integer hash
- [x] 5.3 Add a viewport quality state — interactive, balanced, high — chosen
      by the application from what the pointer is doing, with hysteresis so it
      does not flip per event, and never discovered by the renderer itself
- [x] 5.4 Decide against temporal accumulation, and record why: its condition
      is "to allow cheaper samples", and the quality governor already reaches
      that end without a history to go wrong. The failure mode of the machinery
      it needs — two reprojected ping-pong pairs and a validation rule — is
      occlusion trailing behind a brush, which is the one artefact a sculptor
      cannot work through

## 6. Materials that hold up at a distance

- [x] 6.1 Give MatCaps a mip chain, each level generated from the procedural
      recipe rather than downsampled through gamma
- [x] 6.2 Give reference images linear-correct mipmaps and anisotropy, which
      is the one place in this renderer where anisotropy earns its cost
- [x] 6.3 Add an optional rim term to the MatCap lookup, as a material
      parameter and not a universal effect
- [x] 6.4 Add an optional screen-space cavity term, computed from neighbouring
      reconstructed positions and applied in the composite — subtle by
      default, and off while sculpting

## 7. Depth worth its bits

- [x] 7.1 Derive near and far from camera distance and scene bounds, smoothed
      so the range does not pop, replacing the fixed 0.01/1000
- [x] 7.2 Move the viewport to reversed-Z: near at 1, far at 0, `GreaterEqual`,
      clear to 0, and the wire bias sign reversed
- [x] 7.3 Flip every depth assumption that follows — the AO background test,
      the closest-sample reduction, the capture fixtures — and cover the
      projection round trip, corner rays, framing and near clipping in tests

## 8. Scenes that grow

- [x] 8.1 Carry bounds on each mesh span and cull spans against the camera
      frustum
- [x] 8.2 Grow GPU buffers geometrically rather than to the exact size asked
      for, and never shrink on the interaction path
- [ ] 8.3 Patch vertex-only mesh edits without re-uploading indices — blocked
      on the engine layer saying whether an edit changed topology, since the
      renderer cannot guess it and a wrong guess is a stale index buffer. The
      safe half is taken: the polyframe's edge set is no longer derived on
      every upload when the polyframe is off
- [ ] 8.4 Give voxel chunks persistent GPU slots so a dirty chunk is a write
      into its own range rather than a whole-model upload
- [x] 8.5 Measure CPU draw submission before recording the static overlays
      into render bundles, and record the figure that says not to: four draw
      calls at 1080p

## 9. A presentation mode, beside the sculpt mode

- [x] 9.1 Add a Studio shading mode — key, fill and rim lights over the same
      vertex inputs — selectable beside MatCap and never replacing it
- [x] 9.2 Tone map Studio in the fragment stage, and decide against the HDR
      intermediate: the curve before an sRGB target is the whole benefit of
      tone mapping, and an intermediate buys post-process effects in linear
      high range, of which there are none here. A full-resolution
      `Rgba16Float` target and a second pass to render generated grey clay
      would be bandwidth for nothing, which the review says of it too
- [x] 9.3a Add one fitted directional shadow map inside Studio mode alone,
      allocated only once the rig is asked for
- [ ] 9.3b Add optional environment lighting, once a shadowed rig has been
      looked at and found wanting for it
- [x] 9.4a Order the transparent helpers back to front by camera depth
- [x] 9.4b Decide against weighted-blended OIT, whose condition the review
      states plainly — "only if users actually hit transparency ordering
      failures". A scene holds at most three reference planes, a membrane and a
      ghosted surface, and sorting them back to front is exact for planes that
      do not intersect each other
- [x] 9.5 Add FXAA for when the device will not multisample, never alongside
      it, and switchable because a filter that works on the picture rather than
      on the geometry is a choice rather than an improvement
- [x] 9.6 Make MSAA a selectable quality rather than a constant, resolved to
      what the format actually supports

## 10. Bandwidth, once it is the thing that hurts

- [x] 10.1 Measure whether vertex bandwidth is the limit before packing the
      vertex, and keep the 40-byte layout because it is not: the scene pass is
      0.09 ms at 1080p on 395,392 triangles
- [x] 10.2 Leave GPU-driven indirect draws unimplemented and say why: the
      reference scene submits four draw calls at 1080p

## 11. A renderer that can be read

- [x] 11.1 Split `renderer.rs` along the seams this change already cut —
      overlays, pipelines, ao, textures, shadow, and the quality, profiler and
      frustum modules that came out while the work was going on
- [x] 11.2 Extract the shared WGSL definitions the shaders duplicated, and
      widen the field-math guard from two shaders to the directory
- [x] 11.3 State the pass order as an invariant, and move the scaffolding
      after the occlusion composite so the invariant is true: a manipulator
      standing over a fold was being dimmed by that fold's occlusion

## 13. The cursor, at the weight it is meant to read at

- [x] 13.1 Draw the brush cursor as a camera-facing ribbon of a constant width
      in pixels, rather than as a line list — which WebGPU draws one pixel wide
      whatever the display, and which multisampling has no coverage to resolve

## 12. Say what moved

- [x] 12.1 Report GPU time per pass at each viewport size, at half and full
      occlusion resolution
- [x] 12.2 Write the capture fixtures as relations rather than as golden
      images, since occlusion is a sampled integral and no two drivers agree on
      it to the byte
- [x] 12.3 Update the roadmap and the rendering documentation
