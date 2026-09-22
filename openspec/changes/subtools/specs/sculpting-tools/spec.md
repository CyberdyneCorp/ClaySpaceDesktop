## MODIFIED Requirements

### Requirement: Symmetry is applied about the document axes
The interface SHALL offer symmetry about the X, Y and Z axes, independently toggleable, applied through the engine's mirroring. The active symmetry axes SHALL be visible while sculpting.

Symmetry SHALL be a property of each layer, not of the document: the toggles
read and write the active layer's axes, and switching the active layer SHALL
restore that layer's own setting rather than carrying the previous layer's
along. A new layer starts with X symmetry on, as the design asks.

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

#### Scenario: Symmetry does not leak across a subtool switch
- **WHEN** the user turns symmetry off on one layer, activates another layer
  that has it on, and sculpts
- **THEN** the edit on the second layer is mirrored, and returning to the
  first layer finds symmetry still off

### Requirement: Masks freeze regions against every verb
The application SHALL let the user paint, invert, clear, expand, contract and smooth mask fields, and SHALL pass the active mask to every sculpting verb it invokes. A fully masked region SHALL be unchanged by any verb.

A mask SHALL belong to the layer it was painted on. Each layer MAY carry its
own mask; the mask presented and applied is the active layer's, and switching
the active layer SHALL neither discard the previous layer's mask nor apply it
to the new one.

#### Scenario: A masked region resists every tool
- **WHEN** a region is fully masked and each available sculpting tool is applied over it
- **THEN** no tool alters that region

#### Scenario: Masks survive a resolution change
- **WHEN** the voxel resolution level changes on a layer carrying a mask
- **THEN** the mask still covers the same region of the model

#### Scenario: Two subtools keep independent masks
- **WHEN** the user paints a mask on one layer, activates another and paints a
  different mask there, then returns to the first
- **THEN** the first layer's mask protects exactly what was painted on it, and
  neither mask gates edits on the other layer

### Requirement: A deformation cage bends the whole form
The application SHALL offer a lattice cage around the active layer, sized to
what that layer contains, with control points drawn in the viewport and
draggable directly.

The cage SHALL be worked in rather than applied per drag: the form follows when
the cage is applied, and the whole cage SHALL be one undo step however many
control points were dragged.

The cage SHALL be offered wherever the engine has a route for it, at the
resolution that route accepts, and refused readably where it has none.

A cage belongs to the subtool it was raised around. Changing the active
subtool while a cage stands SHALL resolve it — applied or dropped, as the
sculptor chooses — rather than carrying it to the new subtool, whose form it
was never sized to.

#### Scenario: A cage wraps the form and bends it
- **WHEN** a cage is put around a layer and its top control points are dragged up
- **AND** the cage is applied
- **THEN** the top of the form has moved by the same amount
- **AND** one undo puts it back

#### Scenario: An untouched cage changes nothing
- **WHEN** a cage is put up and applied without dragging anything
- **THEN** the form is unchanged and no history entry is recorded

#### Scenario: A layer with no lattice route says so
- **WHEN** a cage is asked for on a voxel layer
- **THEN** it is refused with a reason naming the crossing that would work

#### Scenario: Switching subtools resolves a standing cage
- **WHEN** a cage is dragged but not applied and the sculptor activates another
  subtool
- **THEN** the sculptor is asked to apply or drop it, and the cage does not
  appear around the newly active subtool

### Requirement: Armatures are authored as a tree
The application SHALL expose armature authoring — a tree of spheres skinned by the engine's sphere-swept links and smooth union — allowing nodes to be added, moved, resized, reparented and removed, with the skin thickness controllable. Moving a parent node SHALL carry its subtree.

An armature SHALL belong to the layer that holds its nodes. A document MAY
carry an armature per layer; activating a layer that carries one SHALL make
that rig the one presented and posable, and rigs on other layers SHALL be
untouched by it.

An edit naming a sphere the rig does not have SHALL be refused and SHALL leave
the history alone. Every rig edit rewrites the whole armature into the
document, so an index nobody has would otherwise place the tree again
unchanged — an undo step for a gesture that never happened, spent instead of
the sculptor's last real one.

The tree SHALL hold the radii as authored and the document SHALL hold them with
the skin thickness applied, so that moving the thickness is reversible and
never rewrites the rig. Reading a rig back out of the document SHALL divide by
the thickness **in effect**, under the same bounds the multiply uses. Dividing
by the default instead returns radii with the thickness baked in, and the next
edit writes them out scaled a second time.

A rig SHALL survive a step through the history, in both directions and past its
own creation: after a step the tree the editor holds SHALL be the one the
document holds, and a rig a step brings back SHALL be posable — accepting a new
sphere, a resize and a reparent — rather than present in the surface and
unreachable. Where a step brings a rig back onto a subtool that is not the
active one, that subtool SHALL become active, since a rig is offered for the
active subtool alone.

A change to the skin thickness SHALL itself be undoable: a step back over it
SHALL restore the thickness as well as the radii it wrote, and a step forward
SHALL apply it again.

#### Scenario: Moving a parent carries the chain
- **WHEN** the user moves an armature node that has descendants
- **THEN** the descendants move with it and the skinned surface follows

#### Scenario: Armatures persist with the document
- **WHEN** a document containing an armature is saved and reopened
- **THEN** the armature tree is present and editable

#### Scenario: Each subtool's rig is its own
- **WHEN** two layers each carry an armature and the user poses one
- **THEN** the other layer's rig and skin are unchanged, and activating the
  other layer presents its rig as it was left

#### Scenario: A rig taken back past its creation comes back editable
- **WHEN** the user undoes past the creation of a rig and then redoes
- **THEN** the rig is present on its subtool, that subtool is the active one,
  and it accepts a new sphere, a resize and a reparent

#### Scenario: Radii survive a step at a thickness other than the default
- **WHEN** the skin thickness is 0.5 and the user undoes and redoes five times
- **THEN** no radius in the tree has changed

#### Scenario: The thickness is itself one step
- **WHEN** the user moves the skin thickness and undoes once
- **THEN** the thickness is what it was, the radii are what they were, and the
  edit before the thickness change is still there to undo

#### Scenario: An edit on a sphere that is not there is refused
- **WHEN** the user resizes or reparents a sphere index the rig does not have
- **THEN** the edit is refused with a notice, the tree is unchanged, and the
  history offers exactly what it did before
