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

#### Scenario: A mirror change leaves no ghost
- **WHEN** a stroke changes a layer's mirror, and the layer is then hidden
- **THEN** the viewport draws neither the reflections the old mirror made nor
  anything of the hidden layer

#### Scenario: Symmetry does not leak across a subtool switch
- **WHEN** the user turns symmetry off on one layer, activates another layer
  that has it on, and sculpts
- **THEN** the edit on the second layer is mirrored, and returning to the
  first layer finds symmetry still off
