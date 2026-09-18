# Design

## What is sampled, and why that resource

Three routes reach a depth-aware overlay. Two were rejected before this one.

**Attach the framebuffer's depth buffer.** The pass would need a pipeline whose
sample count matches the attachment, and the scaffolding is deliberately
single-sampled into the resolved target. So this means either resolving depth —
a new pass and a new texture — or moving the scaffolding back inside the scene
pass, which is the one thing the pass order exists to prevent: occlusion is a
multiply over everything drawn before the composite, and a manipulator standing
over a deep fold came out dimmed by that fold.

**Sample the multisampled depth directly.** Legal, as
`texture_depth_multisampled_2d`, and it forks the pipeline: a device that will
not multisample needs the other binding type and so a second shader and a
second pipeline. This is the exact fork the reduced-depth buffer was introduced
to remove — before it existed, a device that would not multisample got no
occlusion at all.

**Sample the reduced depth.** `Framebuffer::reduced_depth` is already
single-sampled, already `R32Float`, already `TEXTURE_BINDING`, and already
holds the scene's depth for the frame being drawn. It is the resource this pass
needs, built for a neighbouring reason and available.

What it costs is resolution: `AO_SCALE` is two, so a depth texel covers a 2×2
block of display pixels and the boundary between bright and faint can be a
display pixel out. A dim is a low-frequency decision — which half of a ring is
behind a head — and a pixel of slop in where it starts is not perceptible. This
is the same argument the occlusion kernel makes for running at that resolution,
and it is weaker there, since occlusion has to survive an upsample.

The reduction takes the **closest** sample of each block, over every
multisample. So the faint region is biased outward by up to a pixel around the
form's silhouette: a fragment just outside the form can be told the form is
there. Outward, toward reading a hoop as passing behind, which is the harmless
direction.

## Why the reduction has to leave the occlusion gate

`occlude` returns early when occlusion is off, and the reduction is its first
pass. Had this change read the reduced depth without moving it, the manipulator
would have been dimmed only when occlusion was on — a manipulator whose
appearance depends on the occlusion setting, which is what
`occlusion_does_not_darken_the_manipulator` exists to forbid. That test compares
the manipulator's pixels with occlusion on against occlusion off, so it fails on
exactly this mistake. It was left unmodified for that reason.

The reduction now runs when the occlusion needs it **or** when there is
scaffolding to draw, and the kernel and composite still run only for occlusion.
A frame with neither pays nothing.

## The depth comparison

Both sides are raw depth-buffer values in the same reversed-Z convention, so
there is no linearisation and no near/far to get wrong. `DEPTH_COMPARE` is
`GreaterEqual` and the buffer clears to `0.0`, so a larger value is nearer, and
a fragment is behind the sculpt when its own `@builtin(position).z` is **less**
than the depth sampled at its pixel.

`DEPTH_BACKGROUND` is that clear value, and the surface is the only thing that
writes depth — the grid, the symmetry plane, the reference image and the
membrane all draw without it. So "the sampled depth is the background value"
means exactly "the sculpt is not here", and a manipulator over empty space, over
the grid, or over a reference photograph is never dimmed.

## What this does while a cage is up, which is nothing

The ghost pipelines write no depth, and their comment says why: it "is what lets
the far half of the cage read through the form". So whenever the surface is
ghosted — a cage being edited, or the opacity dialled back — the sculpt writes
no depth at all, the sampled value is the background, and the scaffolding is
drawn at full strength exactly as it is today.

That falls out rather than being arranged. The rule can be the single uniform
one — scaffolding behind the sculpt is drawn faint — with no exception list for
the cage, and the deliberate "seen through, not turned off" behaviour is
preserved by the depth buffer being empty rather than by a special case that
could rot.

Where it does apply is a manipulator on a solid form, which is the guide's own
case.

## Where the binding lives

The scaffolding gets a shader module of its own and a bind group of its own,
following the studio shadow's precedent and its recorded reasoning: a bind group
layout is part of a pipeline's layout, so putting this in group 0 would make
every overlay, reference and wireframe pipeline in the viewport carry a depth
texture it never samples.

Group 0 is unusable for a second reason here. Its bind group is built once, and
`reduced_depth` is a view that is replaced on every resize — so group 0 would
have to be rebuilt per framebuffer, which is what the occlusion resources cache
already does correctly. The new bind group joins that cache and is keyed on the
same framebuffer id.

The orientation gizmo keeps a pipeline built against the old layout, with no
depth binding. A flag it could read wrongly is a flag that will one day be read
wrongly; a pipeline with nothing bound cannot dim.

## How faint

`XRAY_ALPHA` is the alpha a fragment behind the sculpt keeps. Too high and there
is no cue; too low and a handle looks disabled rather than distant — and it is
still grabbable, so looking disabled is a lie about what a click will do. The
figure is recorded with what breaks in both directions, beside the AO constants
that carry the same kind of note.
