# The manipulator keeps its size, and says what the numbers are

## Why

Two gaps, one of them an inconsistency between two widgets that look identical.

**A cage's manipulator shrank with the camera and an object's did not.**
`object_gizmo_reach` is a share of the camera's distance, so the manipulator on
a placed object or a whole subtool stays the same size to the hand at any zoom
— its comment says exactly that, and says it replaced a version sized from the
subtool's own box. The cage's manipulator never got the same treatment: it is a
share of the cage's rest span alone. Zoom out from a cage and its manipulator
goes with it, while an identical-looking widget on the object beside it holds
still. Same shapes, same gestures, different behaviour under the same wheel.

**A manipulator cannot say what the numbers are.** It shows that something
moved; whether it moved twelve millimetres or twelve and a half is not
something a widget can show, and it is the first question asked when two
objects have to line up. Nothing in the interface reported a placed object's
position or rotation at all — the object section carries its scale and nothing
else.

## What Changes

- **The cage's manipulator takes the same screen-constant floor** the object's
  has. It may still grow past that on a large cage, because the arms should
  reach past what they turn rather than sit as a mark in the middle of it.
- **A transform readout stands in the viewport's lower-leading corner** while a
  manipulator is pointed at a placed object: its position, its rotation, the
  axis that rotation is about, and its scale. Translucent, so the form it
  describes is not hidden by the description.
- **It is shown for a placed object and nothing else.** A cage's target is a
  set of control points and a layer's is everything it holds; neither has a
  single position, rotation and scale, and a readout showing the pivot instead
  would answer a question nobody asked with a number that looks like the answer
  to one they did.

## What it does not show, and why

The reference this is drawn from lists three rotation values and three scale
values. The engine's transforms take **an axis and one angle**, and **one scale
factor** — `SceneObject` says so, and `GizmoMode::Scale` offers a single handle
for exactly that reason, which this repository's own specification already
states as a requirement. Three rotation rows would be two invented numbers.

## Out of scope, and why

- **A universal manipulator** — translate, rotate and scale offered at once,
  with the press choosing the operation. This is the largest item in the guide
  and it is not presentation: `GizmoMode` is domain state, held per cage and
  per object selection, set by a command and drawn as three chips in two
  panels. Making the mode a property of the handle rather than of the selection
  changes what a press means, what `SetGizmoMode` is for, and what the chips
  say. It deserves its own change with its own argument, not a corner of one
  about sizing.
- **X-ray occlusion for the manipulator.** The guide asks for a depth-tested
  pass and a faint pass over it. There is already a requirement here — *The
  manipulator is seen wherever it stands* — describing how the widget is drawn
  after the occlusion composite and compares `Always`, which is a different and
  deliberate answer to the same problem. Changing it means moving that
  requirement and re-measuring the pass order behind it.
- **Larger hit regions.** Already landed: `ray_hits_segment` gives an arrow a
  capsule to be grabbed by rather than a point at its tip.
- **A drag delta shown near the handle.** The readout reports the transform,
  which is the standing question. What a single drag changed is a different
  feature and wants the gesture's own state.
