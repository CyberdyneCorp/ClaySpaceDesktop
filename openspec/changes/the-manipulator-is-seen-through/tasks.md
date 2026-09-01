# Tasks

## 1. Lift the depth reduction out of the occlusion gate

- [x] 1.1 Split `occlude` so the reduction is its own step, run when occlusion
      needs it *or* there is scaffolding to draw
- [x] 1.2 Write the occlusion uniform whenever either runs — the reduction reads
      the sample count and the background value from it
- [x] 1.3 A frame with neither pays nothing

## 2. The scaffolding's own shader and bind group

- [x] 2.1 A shader module of its own, so its bindings do not collide with the
      studio shadow's group in the scene module
- [x] 2.2 Share the vertex structs through `common.wgsl` rather than copying
      them — that file's own rule
- [x] 2.3 A bind group layout beside the occlusion ones: the reduced depth and a
      uniform of its own
- [x] 2.4 Build the bind group per framebuffer, in the cache that is already
      keyed on the framebuffer's id — `reduced_depth` is replaced on resize
- [x] 2.5 Compare raw reversed-Z depth on both sides; dim when the fragment is
      behind, never when the sampled value is the background

## 3. Leave the orientation gizmo alone

- [x] 3.1 Keep a pipeline with no depth binding for it, rather than a flag that
      says not to dim

## 4. Tests

- [x] 4.1 A ring around a solid form: the part behind is fainter than the part
      in front
- [x] 4.2 With no form, every part is at full strength
- [x] 4.3 Against a ghosted surface, every part is at full strength
- [x] 4.4 The existing pass-order tests unmodified, since one of them is exactly
      the check that the reduction is not gated on occlusion
- [x] 4.5 Mutation-check each new test by putting the bug back

## 5. What the mutation checks turned up

- [x] 5.1 A background test written beside the comparison was **redundant**:
      under reversed Z the cleared value is the smallest one, so "nothing was
      drawn here" and "nothing is in front of this fragment" are one condition.
      Putting the bug back left every test passing, which is what a redundant
      guard looks like from outside. Removed, with the convention it now leans
      on named and the test that holds it verified by inverting the comparison
- [x] 5.2 The first empty-scene test was **vacuous** — it passed the same frame
      as both the blend and the full-strength reference, so its ratio was
      identically 1 and could not fail. It was also redundant with the ghost
      test, which measures the same empty depth buffer and does it correctly.
      Replaced with the orientation gizmo, whose own mechanism nothing covered
- [x] 5.3 The first measure normalised the ring over the form against the ring
      over the background, and those differ because the two backgrounds differ.
      Replaced by recovering the blend factor from three frames, which measures
      the dimming itself
- [x] 5.4 `occlusion_does_not_darken_the_manipulator` failed for a real reason,
      not a threshold one: a faint pixel is part widget and part form, and the
      form's share darkens because it is the form. The invariant is restated
      over the pixels the manipulator covers opaquely — 0 of 5382 darken
- [x] 5.5 Moving `Camera` and `VertexInput` into the prelude broke the scene
      module, which was still created straight from `include_str!`. It builds
      cleanly, because WGSL is compiled when a device is asked for the module.
      `every_shader_module_gets_the_shared_prelude` now reads that off the
      renderer's own source

## 6. Verification

- [x] 6.1 Look at the captures
- [x] 6.2 `just check`
