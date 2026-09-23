# object-transform Specification

## Purpose
The move, turn and scale manipulator on the things a sculptor puts in a scene —
placed objects, whole layers, imported meshes and curves — rather than only on
a deformation cage's control points.

## Requirements

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
of depth: a handle that lies behind or inside the form SHALL still be drawn and
SHALL still be grabbable on the same terms as one in front of it. It MAY be
drawn fainter to say that it is behind — a cue about distance, never about
whether it can be used, and never to the point of being hidden.

The manipulator's handles SHALL be drawn heavier than a single device pixel,
its arrowheads, scale boxes and pivot SHALL be solid shaded bodies rather than
line hints, and its arms SHALL keep a constant size on screen as the camera
moves toward or away from what it acts on. The size drawn and the size
hit-tested SHALL come from one definition, and the strength drawn SHALL have no
bearing on either.

#### Scenario: A manipulator inside the form
- **WHEN** a manipulator's pivot and every handle lie inside a placed sphere
- **THEN** the manipulator is drawn over the sphere's surface, faint where the
  sphere stands in front of it

#### Scenario: A faint handle is still a handle
- **WHEN** a press lands on a handle that is drawn faint because the form is in
  front of it
- **THEN** it is grabbed exactly as a handle drawn at full strength would be

#### Scenario: Zooming keeps the widget the same size to the hand
- **WHEN** the camera moves to half its distance from the selection
- **THEN** the manipulator's arms cover the same fraction of the viewport as
  before, and a press at the drawn tip of an arm still finds that arm

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
A transform gesture SHALL be one entry in the edit history however many frames
it took, and the resulting placement SHALL survive saving and reopening the
document.

A per-axis scale SHALL survive with it. Where the stored format cannot express
one, a document written by a version that could SHALL still open, with the
object read as evenly scaled rather than the row being dropped.

A whole subtool's placement — where it stands, how it is turned and how it is
stretched — SHALL be read back from the engine rather than reconstructed by the
application, both when a document is opened and after the history moves.

#### Scenario: Undoing a layer transform
- **WHEN** a layer is moved and the user undoes once
- **THEN** the layer returns to where it was in one step

#### Scenario: A transform survives a reopen
- **WHEN** a document with transformed objects and layers is saved and reopened
- **THEN** everything is where it was left

#### Scenario: A stretch survives a reopen
- **WHEN** a stretched object is saved and the document is reopened
- **THEN** the object is still stretched by the same factors

#### Scenario: A document written before per-axis scale still opens
- **WHEN** a document written by a version that stored one scale factor is opened
- **THEN** its objects are read as evenly scaled, and no row is dropped

#### Scenario: A moved subtool reopens where it stands
- **WHEN** a document whose subtool was moved, turned and stretched is reopened
- **THEN** the subtool's placement is what it was saved as, and the manipulator
  stands on the form rather than at the origin

#### Scenario: Undoing a stretch takes it back
- **WHEN** a whole subtool is stretched and the edit is undone
- **THEN** the subtool is unstretched, and redoing stretches it again

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

### Requirement: Every part of a handle a person can see can be grabbed
A press on any part of a manipulator arrow — anywhere along the shaft as drawn,
not only its arrowhead — SHALL begin a slide along that axis. A press past
either end of the arrow SHALL NOT.

Where another handle sits on the shaft — the centre, a scale box, or a ring
that crosses the axis — that handle SHALL keep the press. Where a ring passes
*behind* the point pressed, the shaft SHALL take it: the handle nearer the eye
is the one a person aiming at what they can see means.

What is drawn and what can be grabbed SHALL come from one definition, and the
rule SHALL live where a test can reach it rather than in the composition root.

#### Scenario: A press halfway along an arrow moves the selection
- **WHEN** the user presses the vertical arrow halfway between the pivot and
  its arrowhead, clear of the scale box and the rings
- **THEN** the selection slides vertically

#### Scenario: A ring behind the press does not take it
- **WHEN** the user presses a point on the inner shaft, with the far side of an
  axis ring behind it along the same ray
- **THEN** the selection slides rather than turning

#### Scenario: The handles on the shaft keep their presses
- **WHEN** the user presses the scale box, or the centre block at the arrow's
  foot
- **THEN** that handle's operation runs, not a slide along the axis

#### Scenario: A press past the arrowhead grabs nothing
- **WHEN** the user presses well beyond the tip of an arrow, along the same
  axis
- **THEN** no handle is grabbed

### Requirement: The handle a press would take is shown before the press
The manipulator SHALL light the handle under the pointer, answering the same
question a press answers and from the same rule, so that what is highlighted
and what a press grabs cannot describe different widgets. While a drag is under
way the handle in hand SHALL stay lit, wherever the pointer has since
travelled.

#### Scenario: An arrow lights under the pointer
- **WHEN** the pointer rests over a manipulator arrow
- **THEN** that arrow is drawn differently from the rest of the widget

#### Scenario: A drag keeps its handle lit
- **WHEN** a handle is being dragged and the pointer travels off it
- **THEN** the handle in hand stays lit

### Requirement: A cage owns the pointer, cursor and all
While a deformation cage is up, the brush cursor SHALL NOT be drawn over the
form. A ring under the pointer states that the next press leaves a stroke, and
while a cage is up no press does.

The rule SHALL be one rule covering every mode that takes the press away from
the brush — the whole-subtool manipulator and the cage alike.

A raised cage SHALL refuse a stroke on **every** path and not only under the
pointer. The refusal SHALL be stated where the command is applied, so that a
caller which never touched a pointer meets it too, and SHALL name the cage so
the caller knows to apply it or take it down. The pointer's own routing SHALL
remain as a second line rather than the only one.

A raised cage SHALL also take the whole-subtool manipulator's target away. The
cage and the placed object both answer the manipulator's commands and which of
them acts is decided by which one has a target, so a target left standing under
a cage makes one drag change two things.

#### Scenario: No brush is drawn while a cage is up
- **WHEN** a deformation cage is up and the pointer is over the form
- **THEN** no brush cursor is drawn, and a press leaves no stroke

#### Scenario: A stroke asked for while a cage is up is refused
- **WHEN** a stroke is begun on a caged layer by a caller that is not the
  pointer
- **THEN** it is refused naming the cage, nothing is collected, and the layer
  is unchanged

#### Scenario: A cage drag moves only the cage
- **WHEN** a cage is raised over a selected object and the manipulator is
  dragged
- **THEN** the cage's control points move and the object does not

### Requirement: A press that takes hold of nothing gathers control points
While a deformation cage is up, a primary press that grabs neither a
manipulator handle nor a control point SHALL draw a selection box across the
viewport, and on release every control point inside the box SHALL become the
selection — including points standing behind the form, which the viewport
already draws through for that reason.

With the add modifier held, the box's catch SHALL be added to the selection
rather than replacing it. A press and release in one place SHALL be a click on
nothing, which clears the selection; a small movement between them SHALL NOT
turn it into a box.

The selection SHALL be resolved when the pointer is released rather than while
the box is drawn, so the manipulator does not wander to the middle of whatever
is momentarily enclosed.

Turning the camera SHALL remain available while a cage is up, on the secondary
button and under the orbit modifier.

#### Scenario: A box takes a whole face at once
- **WHEN** the user drags a box around one face of the cage
- **THEN** every control point on that face is selected, front and back, and
  the manipulator stands on the middle of them

#### Scenario: A box adds to what is held
- **WHEN** the user drags a box with the add modifier held
- **THEN** the points it catches are added to the selection already held

#### Scenario: A click on nothing puts the manipulator away
- **WHEN** the user presses and releases on empty space without dragging
- **THEN** the selection is cleared and no manipulator is drawn

#### Scenario: The cage can still be looked at from behind
- **WHEN** the user drags with the secondary button, or with the orbit modifier
  held
- **THEN** the camera orbits and no selection box is drawn

### Requirement: The manipulator keeps its size on screen
The manipulator SHALL be sized so that it stays approximately the same size to
the hand whatever the camera's distance, for every target it can be placed on.
A manipulator MAY grow beyond that size to reach past a large target, and SHALL
NOT shrink below it.

The rule SHALL be the same for every target. Two widgets drawn with the same
shapes and worked with the same gestures SHALL NOT answer a zoom differently.

#### Scenario: Zooming out does not shrink the widget
- **WHEN** the camera is moved away from a selection with a manipulator on it
- **THEN** the manipulator's apparent size does not fall below its screen-constant floor

#### Scenario: Every target is sized by the same rule
- **WHEN** a manipulator is placed on a deformation cage and on a placed object at the same camera distance
- **THEN** neither is smaller than the screen-constant floor

### Requirement: The transform is reported beside the manipulator
While a manipulator is pointed at a placed object, the viewport SHALL show that
object's position, its rotation, the axis that rotation is about, and its
scale. The readout SHALL be translucent and SHALL NOT hide the form it
describes.

The readout SHALL report only values the domain holds: an axis and one angle
for rotation, and one factor for scale. It SHALL NOT present three rotation
values or three scale values.

The readout SHALL be shown only where it has an answer. A target that has no
single position, rotation and scale — a set of control points, or a whole layer
— SHALL be given none.

#### Scenario: The numbers are on screen while the widget is
- **WHEN** a manipulator is on a placed object
- **THEN** the object's position, rotation, rotation axis and scale are shown in the viewport

#### Scenario: Nothing is invented for rotation or scale
- **WHEN** the readout is shown
- **THEN** rotation is one angle about one axis, and scale is one factor

#### Scenario: A target with no single transform gets no readout
- **WHEN** the manipulator's target is a whole layer or a set of control points
- **THEN** no transform readout is drawn

#### Scenario: A value that rounds to nothing is not shown signed
- **WHEN** a coordinate is a small negative that rounds to zero at the display precision
- **THEN** it is shown as zero without a sign

### Requirement: Scale is per axis, on a placed object and on a whole subtool alike
The manipulator SHALL offer a scale handle per axis on a target the engine can
scale per axis, and SHALL NOT offer one on a target it can only scale
uniformly.

A placed object is a node and the engine's node transform takes a factor per
axis. A whole subtool is a layer, and the engine's layer transform has taken a
factor per axis since ClayCore ABI 0.74.0. Both therefore carry the three
boxes, and the manipulator SHALL be one widget with the same handles wherever
it stands rather than two widgets chosen by what it is pointed at.

Where a per-axis scale is applied, the application SHALL use the engine's
per-axis call for *every* transform of that target and not only for a stretched
one: each call writes the whole transform, so a uniform call applied to a
stretched target would collapse the stretch.

A world length carried into a target's own frame — a brush radius, a join width
— SHALL be divided by the largest of the three factors rather than by their
mean, so that a gesture never reaches outside the region it named.

The interface SHALL present one factor where the three agree and three where
they differ, so that an evenly scaled target does not read as three numbers.

#### Scenario: Scaling a placed object
- **WHEN** an object is selected and a scale box on an axis is dragged
- **THEN** that axis is stretched and the other two are unchanged

#### Scenario: The centre handle stays uniform
- **WHEN** an object's centre handle is dragged in scale mode
- **THEN** all three axes are scaled by the same factor

#### Scenario: A whole subtool stretches per axis
- **WHEN** the manipulator is pointed at a whole subtool and a scale box on an
  axis is dragged
- **THEN** that axis of the subtool is stretched, the other two are unchanged,
  and the stretch reaches the field the subtool evaluates to

#### Scenario: Per-axis scale on a cage is unaffected
- **WHEN** a lattice selection is scaled
- **THEN** the per-axis handles are still offered, because a cage scales its
  points and does not carry an engine transform

#### Scenario: A move does not unsquash what it moves
- **WHEN** a stretched object or subtool is moved or rotated
- **THEN** its per-axis scale is unchanged

#### Scenario: An evenly scaled object reads as one number
- **WHEN** an object's three scale factors are equal
- **THEN** the interface shows one factor rather than the same number three times

### Requirement: A stretched subtool is refused the deformation cage in words
The engine returns no warps at all for a layer carrying a per-axis scale,
because a cage records its item-to-cage placement as a rigid transform and a
squashed layer needs a general affine map. The application SHALL refuse the
cage on such a subtool with a message naming the stretch, rather than letting
the refusal arrive as a cage that reached nothing.

#### Scenario: Putting a cage on a stretched subtool
- **WHEN** a subtool carrying a per-axis stretch is caged and the cage applied
- **THEN** the application refuses it and names the stretch as the reason

#### Scenario: The same cage on an unstretched subtool
- **WHEN** the stretch is taken back and the same cage applied
- **THEN** it deforms the subtool as it always did

### Requirement: The document format version this build writes is named
The `.clayspace` container version this build writes SHALL be stated in the
code with its reasoning, checked against the pinned engine's own headers, and
readable from a written file rather than asserted. It SHALL appear in the
diagnostics report.

A build older than the one that introduced a minor refuses such a document
rather than misreading it, so the number is the answer to "it will not open
elsewhere".

#### Scenario: A document this build writes
- **WHEN** a document is saved and its header read back
- **THEN** the version in the file is the one the build says it writes

#### Scenario: The pinned engine moves its format
- **WHEN** the vendored engine declares a container or scene minor past what
  the build claims
- **THEN** the test that compares them fails
