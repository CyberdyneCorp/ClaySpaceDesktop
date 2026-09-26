## MODIFIED Requirements

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

The skin thickness SHALL belong to the rig, not to the document: each subtool
that carries a rig SHALL keep its own, a change to it SHALL rewrite that rig
alone, and every rig SHALL be read back through its own thickness. A subtool
without a rig SHALL refuse a thickness, and a thickness equal to the one in
effect SHALL record no step. Each rig's thickness SHALL be saved with the
document and restored when it is reopened.

A sphere SHALL NOT be added with a radius that is not a positive number; such
an edit SHALL be refused with a notice and SHALL record nothing. Inserting a
sphere on a link SHALL be mirrored, when the rig's mirror is on, by whichever
route asks for it.

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

#### Scenario: Each rig keeps its own thickness
- **WHEN** two subtools carry rigs and the user changes one rig's thickness,
  then steps back and forth through the history
- **THEN** the other rig's thickness and radii are unchanged, and activating
  it presents its own thickness

#### Scenario: A rig's thickness is saved with it
- **WHEN** a document whose rig has a thickness other than the default is
  saved and reopened
- **THEN** the rig comes back at that thickness with its authored radii

#### Scenario: A sphere with a negative radius is refused
- **WHEN** a sphere is added with a negative or zero radius
- **THEN** the edit is refused with a notice and the tree is unchanged

#### Scenario: An insert is mirrored
- **WHEN** the rig's mirror is on and a sphere is inserted on a link that has
  a reflection
- **THEN** a sphere is inserted on the reflected link as well
