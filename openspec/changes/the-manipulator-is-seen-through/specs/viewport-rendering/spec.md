## ADDED Requirements

### Requirement: The scaffolding is drawn faint where the sculpt stands in front of it
The scaffolding SHALL be drawn at reduced strength where the sculpt's surface
is nearer to the camera than the scaffolding fragment, and at full strength
everywhere else. A rotate ring around a solid form SHALL therefore read as a
hoop the form passes through rather than as a circle painted on the frame.

This SHALL be done by **sampling** the depth the scene wrote, not by depth
testing: the pass SHALL bind no depth attachment and SHALL keep its place after
the occlusion composite. The value sampled SHALL be the frame's own reduced
depth — single-sampled, so the comparison does not fork on whether the device
multisamples.

Faint SHALL mean faint and nothing else. A dimmed handle SHALL remain drawn,
SHALL remain hit-testable on exactly the same terms as a bright one, and SHALL
NOT be hidden, since the hit test walks handles by ray and ignores depth.

Where the sculpt wrote no depth the scaffolding SHALL be drawn at full strength.
The surface is the only thing that writes depth, so scaffolding over empty
space, over the grid, over a symmetry plane, over a reference image, or over a
**ghosted** surface is unaffected — which is what keeps a deformation cage
"seen through, not turned off" while it is being edited.

The orientation gizmo SHALL NOT be dimmed. It draws in a corner viewport with a
camera of its own, and the scene's depth in those pixels is whatever a sculpt
reaching into that corner happened to write.

#### Scenario: A ring around a solid form
- **WHEN** a rotate manipulator encircles a solid form, so that part of the
  ring is behind it
- **THEN** the part behind the form is drawn fainter than the part in front,
  and both are still drawn

#### Scenario: Nothing behind it to be behind
- **WHEN** the same manipulator is drawn with no form present
- **THEN** every part of it is drawn at full strength

#### Scenario: A ghosted surface dims nothing
- **WHEN** the same manipulator is drawn against a ghosted surface, which
  writes no depth
- **THEN** every part of it is drawn at full strength

#### Scenario: The orientation gizmo is not dimmed by a sculpt in its corner
- **WHEN** the camera is close enough that the form fills the corner the
  orientation gizmo sits in
- **THEN** the gizmo is drawn at full strength

### Requirement: The depth the scaffolding reads does not depend on the occlusion setting
The depth reduction SHALL run when there is scaffolding to draw, whether or not
occlusion is enabled. The occlusion kernel and the composite SHALL still run
only for occlusion, and a frame with neither scaffolding nor occlusion SHALL run
neither.

A manipulator's appearance SHALL NOT depend on whether occlusion is on. Gating
the reduction on the occlusion setting would make it depend on exactly that.

#### Scenario: The same manipulator under both occlusion settings
- **WHEN** a manipulator standing partly behind a solid form is drawn with
  occlusion on and with occlusion off
- **THEN** the pixels it covers are the same in both, dimming included

## MODIFIED Requirements

### Requirement: The pass order decides what occlusion reaches, and is stated
The viewport SHALL draw in a stated order: the opaque scene with the helpers
that lie behind or on it, the multisample resolve, the depth reduction, the
occlusion kernel, the depth-aware upsample multiplied onto the resolved colour,
and then the scaffolding — the lattice cage, an object's outline, the
manipulator and the orientation gizmo — onto that finished frame.

Occlusion SHALL therefore be applied through the depth the sculpt's surface
wrote. Where nothing was drawn it SHALL leave the frame alone, so the grid, the
symmetry planes and the reference planes are not darkened. The scaffolding SHALL
NOT be darkened by occlusion at all, whatever stands behind it: it is drawn
after the composite, because it stands *over* the form rather than on it, and a
manipulator dimmed by the fold it is being aimed at is dimmed exactly where a
sculptor is most likely to be aiming.

The scaffolding's own depth cue is not this. A comparison against the depth the
sculpt wrote is not occlusion, carries none of the occlusion field's shape, and
SHALL be identical whether occlusion is enabled or not.

Where the scaffolding is drawn faint, the form shows through it, and that form
SHALL be shaded as the form is. So the invariant is stated over the pixels the
scaffolding covers **opaquely**: those SHALL be identical under both occlusion
settings. A faint pixel is part widget and part sculpt, and the sculpt's share
of it darkens because it is the sculpt.

The orientation gizmo SHALL NOT be occluded by the sculpt. It is drawn in a
corner viewport with a camera of its own, and the scene's depth buffer says
nothing about it.

#### Scenario: The manipulator is not shaded by the form behind it
- **WHEN** a manipulator is drawn over an occluded fold, and the same frame is
  drawn with occlusion switched off
- **THEN** every pixel the manipulator covers opaquely is identical between the
  two

#### Scenario: A faint pixel shows the form, and the form is shaded
- **WHEN** the same two frames are compared where the manipulator is drawn faint
- **THEN** those pixels may differ, by as much as the form seen through them
  differs

#### Scenario: The sculpt beneath it still is
- **WHEN** the same two frames are compared over the surface rather than over
  the manipulator
- **THEN** the surface is darkened where it closes in on itself

#### Scenario: The orientation gizmo survives a sculpt in its corner
- **WHEN** the camera is close enough that the form fills the corner the
  orientation gizmo sits in
- **THEN** the gizmo is drawn
