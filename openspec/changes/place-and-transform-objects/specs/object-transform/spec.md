## Purpose

The move, turn and scale manipulator on the things a sculptor puts in a scene —
placed objects, whole layers, imported meshes and curves — rather than only on
a deformation cage's control points.

## ADDED Requirements

### Requirement: The manipulator acts on the current selection whatever kind it is
The application SHALL present one manipulator carrying every operation at once
— an arrow, a ring and, where a stretch can be applied per axis, a box on each
axis; an outer ring turning in the screen plane; and a centre — and SHALL apply
it to whichever of these is selected: a placed object, a whole layer, an
imported mesh layer, or a curve's control points. The operation of a drag SHALL
be that of the handle grabbed; the move / turn / scale mode SHALL decide only
what the centre and a press on the clay do, and SHALL follow the handle last
grabbed. The manipulator's arms SHALL reach past the target's own bounds, with
a floor at the screen-constant size and a ceiling that keeps it on screen.

The manipulator SHALL sit on the middle of what it is transforming, an axis
handle SHALL constrain the drag to that axis, and a drag SHALL be resolved from
where it started rather than accumulated across frames — the same rules the
cage's manipulator already holds.

The three modes SHALL be offered as one row of controls wherever a selection
the manipulator acts on exists — beside the object list, in the shapes panel,
and in the cage section — each carrying the shape of its handle, and the row
SHALL be absent when nothing is selected.

#### Scenario: A ring is grabbed while the mode is move
- **WHEN** the mode is move and the user drags an axis ring
- **THEN** the selection turns about that axis, and the mode reads turn afterwards

#### Scenario: The widget encloses the form
- **WHEN** a whole subtool is selected for transformation
- **THEN** the manipulator's arrows reach past the subtool's bounds and its outer
  ring stands outside them

#### Scenario: The mode is changed with only an object selected
- **WHEN** a placed object is selected and no cage is up
- **THEN** the interface offers move, turn and scale, and choosing turn puts
  the manipulator in turn mode

#### Scenario: A placed object is moved along one axis
- **WHEN** the user drags a placed object's vertical axis handle
- **THEN** the object moves only vertically, and the surface it combines with
  follows it

#### Scenario: A whole layer is turned
- **WHEN** a layer is selected and turned a quarter about an axis
- **THEN** everything the layer holds turns with it, about the layer's own
  middle

#### Scenario: An imported mesh is placed after import
- **WHEN** an imported mesh layer is selected and moved
- **THEN** the mesh is drawn in its new position and exports from there

#### Scenario: A curve is moved as a whole
- **WHEN** a curve's control points are all selected and the manipulator is
  dragged
- **THEN** every control point moves together and the swept form follows

### Requirement: A whole-subtool transform is a mode, and moves what is drawn
While the manipulator on a whole subtool is up, a primary press on that
subtool's surface that lands on no handle SHALL perform the mode's free gesture
— a view-plane move, a screen-plane turn, or a uniform scale — rather than a
sculpting stroke, and the brush cursor SHALL NOT be drawn. A press off the
surface SHALL still orbit. After a layer transform, the drawn surface SHALL be
re-meshed both where the layer was and where it is, so that the incremental
picture matches a rebuild.

#### Scenario: A press on the clay moves the subtool
- **WHEN** Mover is chosen for the whole layer and the user drags on the form
  away from the arrows
- **THEN** the form slides with the pointer and no stroke is made

#### Scenario: One scale gesture is bounded
- **WHEN** the user presses the centre handle a hair from the pivot and drags to
  the edge of the viewport
- **THEN** the form is scaled by at most ten times in that gesture

#### Scenario: A transform the cache cannot track is not applied
- **WHEN** a whole subtool is scaled to a size whose surface region the brick
  cache refuses to track
- **THEN** the layer keeps the last transform the cache accepted, the refusal
  is reported, and the drawn surface stays consistent with the field

#### Scenario: The old position is not left behind
- **WHEN** a whole subtool is moved by its manipulator and the viewport re-meshes
  incrementally
- **THEN** no surface remains where the subtool stood, and the result matches a
  full rebuild

### Requirement: The manipulator is seen wherever it stands
The manipulator, the deformation cage, a curve's control polygon and a
selected object's outline SHALL be drawn over the sculpted surface regardless
of depth: a handle that lies behind or inside the form SHALL be as visible as
one in front of it. The manipulator's handles SHALL be drawn heavier than a
single device pixel, its arrowheads, scale boxes and pivot SHALL be solid
shaded bodies rather than line hints, and its arms SHALL keep a constant size
on screen as the camera moves toward or away from what it acts on. The size drawn and the size
hit-tested SHALL come from one definition.

#### Scenario: A manipulator inside the form
- **WHEN** a manipulator's pivot and every handle lie inside a placed sphere
- **THEN** the manipulator is drawn in full over the sphere's surface

#### Scenario: Zooming keeps the widget the same size to the hand
- **WHEN** the camera moves to half its distance from the selection
- **THEN** the manipulator's arms cover the same fraction of the viewport as
  before, and a press at the drawn tip of an arm still finds that arm

### Requirement: Scale is uniform, and the manipulator offers only that
Scale mode SHALL offer uniform scaling and SHALL NOT present per-axis scale
handles for a target the engine can only scale uniformly.

Every transform in the engine's interface takes a single scale factor rather
than one per axis. Three axis handles that quietly do the same thing as the
centre — or nothing at all — would be worse than one honest control, which is
the same reason a combine operation that cannot reach zero is not given a
slider that can.

#### Scenario: Scaling a placed object
- **WHEN** an object is selected in scale mode
- **THEN** the manipulator offers a uniform scale and no per-axis handle

#### Scenario: Per-axis scale on a cage is unaffected
- **WHEN** a lattice selection is scaled
- **THEN** the per-axis handles are still offered, because a cage scales its
  points and does not carry an engine transform

### Requirement: A sculpting stroke is not transformable
The manipulator SHALL NOT act on a sculpting stroke, and an attempt to select
one for transformation SHALL say so rather than doing nothing.

A stroke is a gesture that has finished. Picking one back up is a different
feature with a different question behind it — which of a stroke's samples is
being moved — and answering it by moving all of them silently would be a tool
doing something adjacent to what a sculptor asked for.

#### Scenario: A stroke offers no manipulator
- **WHEN** the user picks a sculpting stroke in the viewport
- **THEN** no manipulator appears, and the interface states that a stroke
  cannot be transformed

### Requirement: A transform is one undo step and survives a reopen
A completed drag SHALL be a single entry in the undo history whatever the
target, and the resulting transform SHALL be written to the document and read
back from it.

#### Scenario: Undoing a layer transform
- **WHEN** a layer is moved and the user undoes once
- **THEN** the layer returns to where it was in one step

#### Scenario: A transform survives a reopen
- **WHEN** a document with transformed objects and layers is saved and reopened
- **THEN** everything is where it was left

### Requirement: A live operand stays interactive while it is dragged
While an object that participates in a boolean is being dragged, the viewport
SHALL show the result of the boolean at the object's current position, and the
interface SHALL remain responsive throughout.

Where the re-evaluation cannot keep up, the application SHALL show the object
moving against the last completed surface and settle when the drag ends, rather
than blocking the drag.

#### Scenario: The cavity follows the drag
- **WHEN** a subtracted object is dragged across the form
- **THEN** the cavity moves with it

#### Scenario: A form too heavy to re-evaluate live
- **WHEN** the surface cannot be re-evaluated within the frame budget
- **THEN** the drag continues at interactive speed and the surface settles when
  it ends
