## MODIFIED Requirements

### Requirement: Symmetry is applied about the document axes
The interface SHALL offer symmetry about the X, Y and Z axes, independently toggleable, applied through the engine's mirroring. The active symmetry axes SHALL be visible while sculpting.

Symmetry SHALL be a property of each layer, not of the document: the toggles
read and write the active layer's axes, and switching the active layer SHALL
restore that layer's own setting rather than carrying the previous layer's
along. A new layer starts with X symmetry on, as the design asks.

Symmetry SHALL mirror what is made while it is on, and SHALL NOT reach items
that already exist. The engine's mirror belongs to the layer and reflects every
item that takes part in it, so each item SHALL be told when it is made whether
it takes part: a stroke's stamps, a curve and a placed object made with
symmetry off SHALL stay out of any mirror the layer is given later. A curve and
a placed object made with symmetry on SHALL be mirrored from the moment they
are made, in the same undo step as the item.

Symmetry turned off SHALL NOT take the reflections away from items made while
it was on. A stroke, a pull, a curve, a placed object or a bake made with
symmetry off SHALL leave the layer's mirror as it stands, since what it makes
stays out of that mirror. A Move or Pinch drag made with symmetry off SHALL
move one side only; while the engine reflects a drag into every image of an
item that takes part, that drag turns the layer's mirror off.

An armature SHALL stay out of its layer's mirror, since it mirrors itself: a
stroke made with symmetry on on a rig's subtool SHALL NOT give a sphere added
one-sided a twin.

A mirror change SHALL leave no stale surface. The reflections the old mirror
made and the ones the new mirror makes SHALL both be re-evaluated, whether or
not they lie inside the region of the edit that changed it.

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
  the user sculpts, pulls, smooths or flattens on one side
- **THEN** the lump still has its twin, and the new edit has none

#### Scenario: A drag with symmetry off moves one side
- **WHEN** symmetry is off and the user drags one side of a form made under X
  symmetry with Move
- **THEN** the far side of the plane does not move

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
- **WHEN** a stroke changes a layer's mirror to another axis, and the layer is
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
