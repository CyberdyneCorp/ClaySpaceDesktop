## MODIFIED Requirements

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
  follows it — during the drag for an operand that keeps the live path, and
  on release for a union in a field, which is drawn as its own surface while
  the hand moves it

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

### Requirement: A live operand stays interactive while it is dragged
While an object that participates in a boolean other than a union with an SDF
layer is being dragged, the viewport SHALL show the result of the boolean at
the object's current position, and the interface SHALL remain responsive
throughout.

Where the re-evaluation cannot keep up, the application SHALL show the object
moving against the last completed surface and settle when the drag ends, rather
than blocking the drag.

A placed object whose operation is union (`Add`) in an SDF layer SHALL instead
be drawn as its own surface where the hand has taken it, with the field and its
blend written once on release, as "A placed object's drag in a field draws the
object until release" in `viewport-rendering` states. Every other operation —
subtraction, intersection and the rest of the list — keeps the live path.

#### Scenario: The cavity follows the drag
- **WHEN** a subtracted object is dragged across the form
- **THEN** the cavity moves with it

#### Scenario: A form too heavy to re-evaluate live
- **WHEN** the surface cannot be re-evaluated within the frame budget
- **THEN** the drag continues at interactive speed and the surface settles when
  it ends

#### Scenario: A union object is drawn alone until release
- **WHEN** a placed object combined by union in an SDF layer is dragged
- **THEN** its own surface follows the hand while the rest of the field stays
  as drawn, and its blend with the form appears on release
