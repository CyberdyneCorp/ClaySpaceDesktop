## MODIFIED Requirements

### Requirement: Symmetry is applied about the document axes
The interface SHALL offer symmetry about the X, Y and Z axes, independently toggleable, applied through the engine's mirroring. The active symmetry axes SHALL be visible while sculpting.

Symmetry SHALL be a property of each layer, not of the document: the toggles
read and write the active layer's axes, and switching the active layer SHALL
restore that layer's own setting rather than carrying the previous layer's
along. A new layer starts with X symmetry on, as the design asks.

#### Scenario: Mirrored edits are symmetric
- **WHEN** X symmetry is active and the user sculpts on one side
- **THEN** the mirrored edit is applied on the other side within the same edit, and undoing the edit removes both

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
