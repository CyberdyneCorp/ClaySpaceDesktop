# The manipulator is seen through the form it stands on

## Why

The redesign guide's last unbuilt item, and its criticism is fair: a rotate
ring drawn wholly on top gives no sense of which half is nearer. Every arm of a
manipulator is drawn at full strength wherever it is, so a ring around a solid
subtool reads as a flat circle painted on the frame rather than as a hoop the
form passes through.

It was left twice, and the second time with the blocker written down. Both of
the reasons first given for leaving it were wrong, and both were established as
wrong before this change started:

- It is **not** blocked by the pass-order requirement. That requirement forbids
  the manipulator being *darkened by occlusion*; a depth comparison is not
  occlusion. Its scenarios assert the manipulator's pixels are identical with
  occlusion on and off, and they still hold here — which is exactly why the
  reduction this change relies on must not be gated on the occlusion setting.
- It costs **no** grabbability. The hit test walks every handle by ray and
  ignores depth entirely, so a handle drawn faint is as easy to grab as one
  drawn bright. Faint is a cue, not a state.

What actually blocked it was concrete: the pass that draws the manipulator
binds no depth attachment — `depth_stencil_attachment: None` — and runs after
the multisample resolve, so the scene's depth is multisampled where this pass
is not. Attaching it means resolving depth or moving the draw back before the
composite, and moving it back is the thing the pass order exists to prevent.

## What Changes

- **The scaffolding samples the scene's depth and draws faint where it is
  behind.** Sampling rather than testing: no depth attachment is added, the
  pass keeps its place after the composite, and nothing about the pass order
  changes.
- **What it samples already exists.** `Framebuffer::reduced_depth` is
  single-sampled `R32Float` at half resolution, already carries
  `TEXTURE_BINDING`, and was created to free the occlusion kernel from
  multisampling — which is the same problem this pass has. No new attachment,
  no depth resolve, no second depth buffer.
- **The reduction runs whenever the scaffolding needs it**, not only when
  occlusion is on. It used to return early with the rest of the occlusion work.
  Left that way, a manipulator would be dimmed only while occlusion happened to
  be enabled — which is precisely what the pass-order invariant forbids, and
  its existing test is what would have caught it.
- **The orientation gizmo is left out.** It draws in its own corner viewport
  with its own camera; the scene's depth in those pixels is whatever the sculpt
  reaching into that corner happened to write. It was freed from exactly that
  once already. It keeps a pipeline with no depth binding at all, so it cannot
  be dimmed by construction rather than by a flag being read correctly.

## Impact

- `clayspace-view`: a shader module of its own for the scaffolding, a bind
  group beside the occlusion ones, and the depth reduction lifted out of the
  occlusion gate.
- No domain change, no command, no setting. This is presentation.
