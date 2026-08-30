# Design

## The shape of the thing

The renderer does not change shape. It stays a forward pass with a MatCap over
it, multisampled, writing `Depth32Float`, with occlusion derived from its own
depth buffer and no G-buffer anywhere. What changes is where each pass runs, what
it is allowed to assume, and who decides how much a frame is worth.

```text
                        SCENE GEOMETRY
                              │
                  ┌───────────┴───────────┐
                  │ frustum cull per span │   ← new
                  └───────────┬───────────┘
                              ▼
                 ┌────────────────────────┐
                 │ Opaque sculpt pass     │
                 │  MatCap  OR  Studio    │   ← Studio is new
                 │  reversed-Z            │   ← new
                 │  no blending           │   ← new
                 │  2× / 4× MSAA          │   ← selectable, new
                 └───────────┬────────────┘
                             │ resolve colour
                             ▼
                 ┌────────────────────────┐
                 │ Depth reduction        │   ← new pass
                 │  closest covered sample│
                 │  full → half, 1 sample │
                 └───────────┬────────────┘
                             ▼
                 ┌────────────────────────┐
                 │ Occlusion kernel       │
                 │  at half resolution    │   ← was full
                 │  integer-hash rotation │   ← was a sine
                 │  radius ∝ form radius  │   ← was absolute
                 │  samples ∝ quality     │   ← was fixed at 16
                 └───────────┬────────────┘
                             ▼
                 ┌────────────────────────┐
                 │ Depth-aware upsample   │   ← was a 4×4 box
                 │  + optional cavity     │   ← new
                 │  multiplied on         │
                 └───────────┬────────────┘
                             ▼
                 ┌────────────────────────┐
                 │ Transparent helpers    │
                 │  back to front         │   ← new
                 └───────────┬────────────┘
                             ▼
                    wire / cursor / gizmo
                             ▼
                            egui
```

## Decisions worth stating

### Occlusion runs at half resolution and comes back through a depth-aware filter

The kernel is the expensive half of occlusion and it does not need display
resolution: what it computes is low-frequency. What it *does* need is an
upsample that knows where the edges are, because the thing a resolution drop
breaks is exactly the thing a box blur was already breaking — occlusion crossing
a silhouette.

So the upsample weighs each neighbour by how close its depth is to the pixel
being shaded, in view units rather than in raw depth, and the reduction that
feeds it keeps the **closest** covered sample of each block rather than the
average. An average of a foreground and a background that met at a silhouette
describes a surface halfway between them, which is not there and occludes
nothing.

`visual_ao_quality.rs` is the fixture set that holds this: a deep crease, a thin
gap, a contact shadow, a silhouette, and the same form at a hundredth and a
hundred times its size. The silhouette case asserts that **no** background pixel
more than five display pixels from the outline darkens at all — five being the
reach the passes can legitimately have seen the foreground over, worked out from
the block size and the filter's support rather than tuned until it passed.

### The reduction is what frees occlusion from multisampling

Occlusion used to bind the scene's depth buffer directly. A multisampled depth
texture can only be bound as `texture_depth_multisampled_2d`, so a device that
would not multisample got no occlusion at all — a rendering feature switched off
by a *binding*. The reduction pass writes a single-sampled target, so everything
after it is indifferent to the sample count.

The one shader that still binds the scene's depth is written for the
multisampled case and rewritten for the single-sampled one by replacing the
type name. WGSL has no preprocessor and a texture's sample count is part of its
type; the alternative is a second copy of a three-hundred-line shader kept in
step by hand.

### Reversed-Z, and why it is not just precision hygiene

The fixed `near = 0.01, far = 1000` was two failures at once. A thumbnail-sized
import zoomed into is clipped away by a near plane larger than the model; a
large one gets a depth buffer whose whole useful precision is spent on the first
hundredth of the range.

Deriving the range from the viewing distance and the scene's radius fixes the
first. Reversing it fixes the second: floating point crowds its precision near
zero, and a conventional mapping spends that on the far plane where nothing
needs it.

The two are separable and were done separately, but the *fixture* that proves
either is the scale pair — the same fold at ×0.01 and ×100. Before: 0.0% and
0.1% of the form darkened. After the radius became a fraction: 2.9% and 13.8%.
After the depth range followed the scene: **2.9% and 2.9%**.

### The renderer is told what a frame is worth, never asked

`crates/clayspace-view/src/quality.rs` owns the tiers and the hysteresis; the
application owns what the pointer is doing and hands the answer over. A renderer
that worked it out for itself would be a second definition of "is the user
sculpting" for the two to disagree over.

The hysteresis matters more than the tiers. Raising quality on every pointer
release would rebuild the frame at full cost *between two dabs of one stroke* —
which puts the cost exactly where the latency is measured. So the fall is
immediate and the rise waits 160 ms for a settle and 600 ms for idle.

### A profile is a ceiling, not a target

Presentation still drops to the interactive tier under the pen. Performance
never leaves it. This is the rule that keeps a presentation setting from
becoming a latency setting.

### Studio shading is a second mode and not a replacement

MatCap stays the default and stays right: one fetch, stable under a moving
camera, and form reads from it better than from any rig. The one question it
cannot answer is how a form takes a light that stays where it is — its lighting
is welded to the camera, so orbiting the sculpt orbits the light with it.

The studio rig is therefore fixed in the **world**: three lights and an ambient,
their directions taken into view space through the camera's own rotation, with
an ACES curve over the result. `visual_studio.rs` asserts the difference that
matters — the studio highlight travels fourteen times as far across the form
under the same orbit as the MatCap one.

There is no HDR intermediate. The curve is applied in the fragment shader
before the sRGB target encodes it, which is the whole benefit of tone mapping;
an HDR intermediate buys the ability to run *post-process* effects in linear
high range, and there is no such effect here. Adding a full-resolution
`Rgba16Float` target and a second pass to render generated grey clay would be
bandwidth for nothing, which is what the review says about it too.

### Culling is on the CPU, per subtool, against a box

The comment saying a handful of draw calls is noise was right, and stays right,
for a scene holding a handful. At fifty subtools every span is a draw, a bind
and a full pass over geometry outside the frame. Six plane tests against a box
is cheap enough that it does not need a GPU pass or an indirect draw to justify
itself, and the alternative is a great deal of machinery for a scene that has not
been shown to need it.

The test is deliberately conservative: a span with no bounds is never culled,
because a wrong cull is a hole in the frame and a wrong draw is a draw call.

## What is deliberately not here

**GPU-driven indirect draws.** A scene of a handful of subtools submits four
draw calls at 1080p. There is nothing to reduce.

**Render bundles for the static overlays.** Same measurement. The review's own
condition — "only after CPU render submission is measurable" — is not met.

**Packed vertices.** The review's condition is a measurement showing vertex
bandwidth is the limit. The scene pass is 0.08 ms at 1080p on 395,392 triangles;
the limit is elsewhere.

**Persistent voxel GPU chunk slots, and vertex-only mesh patches.** These need
the engine layer to say *which* chunks changed and to hold a stable layout
across syncs, which `visible_mesh_geometry` does not currently express. The
renderer side is ready — `patch_vertices` and `patch_indices` have been there
since the SDF path was written, and buffers now grow geometrically so a patch
does not reallocate — but throwing the switch means guessing whether topology
changed, and a wrong guess is a stale index buffer, which is a wrong picture.
What was done instead is the safe half: the polyframe's edge set, a hash set
over three entries per triangle, is no longer derived on every upload when the
polyframe is off — which it is, by default, for most of most sessions.
