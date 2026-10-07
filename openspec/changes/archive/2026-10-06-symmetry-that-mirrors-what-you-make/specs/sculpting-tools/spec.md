## MODIFIED Requirements

### Requirement: Symmetry is applied about the document axes
The interface SHALL offer symmetry about the X, Y and Z axes, independently toggleable, applied through the engine's mirroring. The active symmetry axes SHALL be visible while sculpting.

Symmetry SHALL be a property of each layer, not of the document: the toggles
read and write the active layer's axes, and switching the active layer SHALL
restore that layer's own setting rather than carrying the previous layer's
along. A new layer starts with X symmetry on, as the design asks.

Symmetry SHALL mirror what is made while it is on, and SHALL NOT reach items
that already exist. Every item SHALL carry the mirror axes symmetry had when
it was made — none when it was off — and SHALL be reflected through those
whatever the layer's mirror is set to afterwards: turning symmetry on, off or
to another axis SHALL leave a stroke's stamps, a curve and a placed object
exactly as they were, and a saved document SHALL reopen with each item's own
axes. A curve and a placed object made with symmetry on SHALL be mirrored
from the moment they are made, in the same undo step as the item.

The layer's own mirror SHALL be written only by the drag verbs, Move and
Pinch, inside their own gesture, and SHALL reach only the items that inherit
it: the starting form, the base shape of a subtool inserted as a shape, and
the items of a document saved before items carried their own axes. A stroke,
a pull, a curve, a placed object or a bake SHALL leave the layer's mirror as
it stands.

A Move or Pinch drag made with symmetry off SHALL move one side only of the
starting form and of items made with symmetry off. An item made under
symmetry is one item with its reflections, and a drag that reaches it SHALL
move its reflections with it, whatever symmetry the drag is made with.

An armature SHALL stay out of its layer's mirror, since it mirrors itself: a
stroke made with symmetry on on a rig's subtool SHALL NOT give a sphere added
one-sided a twin.

A change of the layer's mirror SHALL leave no stale surface. The reflections
the old mirror made and the ones the new mirror makes SHALL both be
re-evaluated, whether or not they lie inside the region of the gesture that
changed it; an item carrying its own axes is not moved by the change and
SHALL cost nothing on it.

A mirrored gesture SHALL move each side exactly as far as the same gesture
moves one side unmirrored. Some verbs are mirrored by the application, which
reflects the gesture and calls the verb once per image, and some are mirrored
by the engine, which reflects a drag "into every image the layer emits of it";
where both would apply, the result is one pull per side rather than two. A
doubled pull is symmetric, so it cannot be found by comparing the two sides
against each other — only against an unmirrored gesture.

#### Scenario: Mirrored edits are symmetric
- **WHEN** X symmetry is active and the user sculpts on one side
- **THEN** the mirrored edit is applied on the other side within the same edit, and undoing the edit removes both

#### Scenario: A mirrored gesture is applied once per side
- **WHEN** the same drag is made with X symmetry on and with it off
- **THEN** each side of the mirrored drag has moved as far as the single side
  of the unmirrored one

#### Scenario: Symmetry off leaves prior work untouched
- **WHEN** the user disables symmetry
- **THEN** existing geometry is unchanged and only subsequent edits are asymmetric

#### Scenario: Turning symmetry off keeps the twins
- **WHEN** a lump is sculpted with X symmetry on, symmetry is turned off and
  the user sculpts, pulls, smooths, flattens, drags or pinches on one side
- **THEN** the lump still has its twin, and the new edit has none

#### Scenario: Switching the axis leaves prior work untouched
- **WHEN** a lump is sculpted with X symmetry on, symmetry is moved to Z and
  the user sculpts and drags
- **THEN** the lump still has its X twin and gains no Z twin, and the new
  stroke is mirrored across Z alone

#### Scenario: Own axes survive a reopen
- **WHEN** a lump sculpted under X and a lump sculpted with symmetry off are
  saved, the document is reopened and a drag is made with Z symmetry
- **THEN** the first lump still has its X twin and the second still has none

#### Scenario: A drag with symmetry off moves one side of the starting form
- **WHEN** symmetry is off and the user drags one side of the starting form
  with Move
- **THEN** the far side of the plane does not move, and a lump made under X
  elsewhere on the form keeps its twin

#### Scenario: A drag reaches an item made under symmetry through its twin
- **WHEN** symmetry is off and the user drags a lump that was sculpted under
  X symmetry
- **THEN** the lump and its twin move together

#### Scenario: Turning symmetry on leaves prior work untouched
- **WHEN** a lump is sculpted with symmetry off, symmetry is turned on and the
  user sculpts again
- **THEN** the lump has no twin

#### Scenario: A placed object is not duplicated by a later symmetry change
- **WHEN** an object is placed with symmetry off and symmetry is turned on
- **THEN** the object is still one object

#### Scenario: A curve begun with symmetry on is mirrored
- **WHEN** a curve is laid on a layer with symmetry on
- **THEN** its reflection is placed with it, in one undo step

#### Scenario: A stroke under symmetry does not mirror a one-sided ZSphere
- **WHEN** a sphere is added to a rig one-sided and a stroke is then made with
  X symmetry on on the rig's subtool
- **THEN** the sphere has no twin, before and after a later rig edit

#### Scenario: A mirror change leaves no ghost
- **WHEN** a drag changes a layer's mirror to another axis, and the layer is
  then hidden
- **THEN** the viewport draws neither the reflections the old mirror made nor
  anything of the hidden layer

#### Scenario: Symmetry does not leak across a subtool switch
- **WHEN** the user turns symmetry off on one layer, activates another layer
  that has it on, and sculpts
- **THEN** the edit on the second layer is mirrored, and returning to the
  first layer finds symmetry still off

## ADDED Requirements

### Requirement: A rig edit keeps the rig's place in its layer
A rig edit SHALL leave everything else on the rig's subtool as it was. A layer
is evaluated in the order of its items, so an armature that is removed and
placed again SHALL be put back where it stood in that order, in the same undo
step as the edit; appended at the end instead, it is combined after every
stroke made on its subtool since, and a carve into the rig is filled in.

#### Scenario: A rig edit keeps a carve made into the rig
- **WHEN** the user carves into a rig on its own subtool, adds a stroke beside
  it, and then moves a sphere of the rig
- **THEN** the carve and the stroke are both still there, and one undo takes
  back the move alone
