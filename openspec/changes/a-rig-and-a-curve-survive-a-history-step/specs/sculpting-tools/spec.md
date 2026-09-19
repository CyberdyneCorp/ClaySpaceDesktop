## MODIFIED Requirements

### Requirement: Armatures are authored as a tree
The application SHALL expose armature authoring — a tree of spheres skinned by the engine's sphere-swept links and smooth union — allowing nodes to be added, moved, resized, reparented and removed, with the skin thickness controllable. Moving a parent node SHALL carry its subtree.

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

### Requirement: A curve places a tube that can be edited afterwards
The application SHALL let a sculptor place a curve by putting control points
down, move those points afterwards, and sweep a tube along it.

Editing a control point SHALL replace the swept form rather than adding
another, and abandoning the curve SHALL take its form with it.

The control points the sculptor holds SHALL follow the history. A point reaches
the engine as part of the guide as soon as there are two to sweep along, so a
step through the history moves them: after any step the points in hand SHALL be
the points the document holds, and a point a step took back SHALL NOT reappear
when the next one is placed. Where a step goes past the sweep's own creation
the curve SHALL be left in hand and empty rather than abandoned, and a step
forward SHALL bring back the same tube rather than place a second one beside
it. Where a step takes away the layer the curve was being drawn into, the curve
SHALL be let go of.

#### Scenario: A tube follows its control points
- **WHEN** a control point of a placed curve is moved
- **THEN** the tube follows it
- **AND** the layer holds one swept form, not one per move

#### Scenario: A curve needs two points to sweep along
- **WHEN** a curve has one control point
- **THEN** nothing is swept

#### Scenario: Abandoning a curve leaves nothing behind
- **WHEN** a curve is taken down without being applied
- **THEN** the form is exactly as it was

#### Scenario: The points in hand follow a step through the history
- **WHEN** the user places three control points and undoes once
- **THEN** the curve in hand holds two points, and redoing brings the third
  back where it was

#### Scenario: An undone point does not come back on the next edit
- **WHEN** the user undoes a control point and then places a different one
- **THEN** the curve holds the points it had before the undone one, plus the
  new one, and nothing else
