# scene-and-layers Specification

## Purpose
The document as a sculptor navigates it — the scene tree, the layer stack, what
each layer costs, what protection means and what it forbids, and what a mesh
layer or an SDF layer's objects can and cannot be asked to do.

## Requirements

### Requirement: The scene is presented as a navigable tree
The application SHALL present the document's objects and groups as a tree reflecting the engine's node structure, showing each entry's name and visibility, and allowing entries to be expanded, collapsed and selected.

#### Scenario: The tree reflects the document
- **WHEN** a group is created or removed through any path
- **THEN** the tree shows the change without requiring the user to refresh it

#### Scenario: Selecting in the tree selects in the viewport
- **WHEN** the user selects an entry in the scene tree
- **THEN** the corresponding geometry is indicated as selected in the viewport

### Requirement: Layers are presented as an ordered stack
The application SHALL present layers as an ordered stack, showing each layer's name, visibility, protection state and intensity, with the evaluation order matching the engine's ordered edit list. The user SHALL be able to create, rename, reorder and remove layers.

#### Scenario: Reordering changes evaluation order
- **WHEN** the user moves a layer above another
- **THEN** the document is re-evaluated with the new order and the viewport reflects the result

#### Scenario: Removing a layer is undoable
- **WHEN** the user removes a layer and undoes the removal
- **THEN** the layer returns with its content, position in the stack, and settings intact

### Requirement: Layer protection states are distinct and enforced
The application SHALL expose the engine's three protection states: visible, ghosted (shown, not pickable, not editable) and locked (shown, pickable, not editable). Attempting to edit a protected layer SHALL be refused with a stated reason rather than silently ignored.

#### Scenario: A ghosted layer is not picked
- **WHEN** the user clicks on geometry belonging to a ghosted layer
- **THEN** the pick passes through to whatever is behind it, and the ghosted layer is not selected

#### Scenario: Editing a locked layer is refused with a reason
- **WHEN** the user applies a brush to a locked layer
- **THEN** no edit occurs and the interface states that the layer is locked

### Requirement: Selection is driven by engine picking
Clicking in the viewport SHALL select through the engine's attributed raycast, resolving to the layer and item under the pointer, honoring ghost and lock states. Selection SHALL be reflected consistently in the viewport, the scene tree and the layer stack.

Selecting a layer — by clicking its geometry in the viewport or its row in the
stack — SHALL make it the active sculpt target: subsequent brush strokes land
on that layer, and tool availability follows its representation. The two ways
of selecting SHALL agree; there is one active layer, not a picked one and a
sculpted one.

#### Scenario: A click identifies layer and item
- **WHEN** the user clicks on a surface
- **THEN** the layer and item the engine attributes to that hit become the selection

#### Scenario: Clicking empty space clears the selection
- **WHEN** the user clicks where the ray hits nothing
- **THEN** the selection is cleared rather than left on the previous target

#### Scenario: Clicking a subtool makes it the sculpt target
- **WHEN** two layers each hold geometry, the first is active, and the user
  clicks the second layer's geometry and then sculpts on it
- **THEN** the dab lands on the second layer and the first is unchanged

#### Scenario: A ghosted subtool does not take the activation
- **WHEN** the user clicks where a ghosted layer's geometry stands in front of
  an ordinary layer's
- **THEN** the ordinary layer behind it becomes active, as the pick already
  passes through ghosts

### Requirement: Layer visibility and transform are directly editable
The user SHALL be able to toggle a layer's visibility and set its transform, applied through the engine's layer operations. A hidden layer SHALL contribute nothing to the displayed surface and SHALL NOT be pickable.

A layer's transform SHALL be settable with the manipulator as well as by any
numeric control the interface offers, and the two SHALL address the same value:
a layer moved by dragging reads back as moved.

Symmetry SHALL follow the layer. The engine reflects a layer's items through
the plane where the local coordinate is zero, and the layer transform carries
that plane with it, so a mirrored layer that is moved stays mirrored about
itself rather than about where it used to be.

Changing a layer's visibility SHALL re-evaluate the field only where the field
can have changed. The surface cache holds the fold of the visible field layers
and nothing else; a grid's and a carried mesh's visibility is honoured where
the drawn geometry is assembled. So showing or hiding a layer that is not a
field layer SHALL re-evaluate nothing, however much material it holds, and
SHALL still leave it out of what is drawn.

Stepping through the history over a visibility change SHALL re-evaluate the
layers whose visibility that change moved, and SHALL NOT stand in for them with
the active layer. A subtool that a step gives back to the document SHALL be
given back to the drawn surface in the same step.

#### Scenario: Hiding removes contribution
- **WHEN** the user hides a layer that contributes to the surface
- **THEN** the viewport shows the surface without that layer's contribution

#### Scenario: Showing a layer again restores the same surface
- **WHEN** a field subtool is hidden and shown again
- **THEN** the surface over it is the one it had before it was hidden

#### Scenario: A grid's eye costs no field work
- **WHEN** a grid or mesh subtool is hidden or shown
- **THEN** no brick of the field is re-evaluated
- **AND** the subtool leaves, or returns to, the geometry handed to the viewport

#### Scenario: Undoing a solo brings back every subtool it hid
- **WHEN** a subtool is soloed and the sculptor then undoes
- **THEN** every subtool the solo hid is both back in the stack and back on the
  drawn surface, not only the one that was active

#### Scenario: A transform is undoable as one step
- **WHEN** the user sets a layer transform and undoes it
- **THEN** the transform reverts in a single undo step

#### Scenario: A mirrored layer is moved
- **WHEN** a layer with symmetry on is moved sideways
- **THEN** its two halves stay symmetric about the layer's own plane

### Requirement: A layer's cost is inspectable
The application SHALL let the user inspect what a layer's field costs, using the engine's field report, and SHALL present the engine's consolidation estimate before offering to consolidate. It SHALL NOT consolidate without the user asking.

#### Scenario: Cost is shown before consolidation
- **WHEN** the user opens consolidation for a layer
- **THEN** the engine's estimated cost is shown and no consolidation runs until the user confirms

#### Scenario: Consolidation is undoable
- **WHEN** the user consolidates a layer and undoes it
- **THEN** the layer returns to its unconsolidated edit list

### Requirement: Geometry statistics are displayed for the current document
The application SHALL display the current polygon, vertex and triangle counts and the object count for the document, updating after edits that change them.

#### Scenario: Counts follow edits
- **WHEN** an edit changes the meshed geometry
- **THEN** the displayed counts update to the new values

#### Scenario: Counts describe what is displayed
- **WHEN** counts are shown alongside a viewport displaying a reduced level of detail
- **THEN** the counts state which resolution they describe, so a reduced LOD is not read as a smaller model

### Requirement: Mesh layers are carried and sculpted, but do not compose
The application SHALL allow an imported mesh to be carried by the document as a
mesh layer, saved and reloaded with it, and exported alongside sculpted content.
A mesh layer SHALL be sculptable in place with the engine's fixed-topology
brushes — see the `mesh-sculpting` capability — and SHALL be pickable in the
viewport.

A mesh layer SHALL NOT be usable as a *live* operand — an entry in another
layer's edit list, an operand of a blend, or the target of a deformer belonging
to another layer. Where a user asks for one, the application SHALL state that
composing requires conversion and SHALL name what that conversion costs.

A **resolved** boolean between two whole subtools is not that, and SHALL accept
a mesh layer. It samples each operand into a volume of its own and stands the
result in a new subtool, so the crossing a mesh needs is performed inside the
operation rather than demanded of the sculptor beforehand, and it is priced with
the same cost vocabulary as any other crossing. See the `subtool-booleans`
capability, which owns that operation and its refusals.

This replaces the previous rule that sculpting tools are disabled on mesh
layers. That rule described the engine as it was: a mesh layer's triangles were
read-only, and the only way to edit one discarded the edge loops and UVs that
made it worth importing. Sixteen fixed-topology brushes now reach a mesh layer's
own vertices without touching its topology, so the refusal describes nothing
that is still true. What has not changed is *live* composability, and that is
the line the requirement now draws — it is also the line that had gone missing,
which is why one spec said a mesh is never an operand while another said every
representation is one.

#### Scenario: A mesh layer round-trips
- **WHEN** a document containing an imported mesh layer is saved and reopened
- **THEN** the mesh layer is present with its geometry unchanged

#### Scenario: A mesh layer is sculpted
- **WHEN** a mesh layer is active and the user selects a mesh brush
- **THEN** the brush is available and a stroke moves the mesh's vertices

#### Scenario: Composing with a mesh layer is refused with a route
- **WHEN** the user asks to subtract a mesh layer from another layer as a live
  edit in that layer
- **THEN** the application states that a mesh layer is not a live operand,
  offers the conversion that would make it one, and names what the conversion
  costs

#### Scenario: A resolved boolean takes the mesh layer as it stands
- **WHEN** the user subtracts one whole subtool from another and one of the two
  is a mesh layer
- **THEN** the boolean runs, having priced the crossing, and the sculptor was
  not asked to convert the mesh beforehand

### Requirement: An SDF layer's objects are addressable
An SDF layer's contents SHALL be presentable as a list of the objects it holds,
each of which can be selected, and selecting one in that list SHALL select it in
the viewport and the reverse.

An item that is not an object a sculptor placed — a sculpting stroke, an
armature's skin, a curve's swept form — SHALL be distinguishable from one that
is, so that a list of a worked layer's contents is not a hundred rows of
"stroke".

#### Scenario: Placed objects are listed
- **WHEN** a layer holds two placed objects and forty strokes
- **THEN** the two objects are listed and reachable, and the strokes do not
  each take a row

#### Scenario: Selection agrees between the list and the viewport
- **WHEN** an object is picked in the viewport
- **THEN** the same object is selected in the list, and a manipulator appears
  on it

### Requirement: A subtool that has become costly to evaluate says so
A field subtool SHALL report what its edit list costs to evaluate and whether
the engine advises collapsing it, and the interface SHALL offer that collapse
while the advice stands.

The application SHALL NOT collapse a layer on its own. Collapsing costs seconds
and changes what the layer holds, so it is offered and never taken quietly.

Reporting the advice SHALL NOT cost what acting on it would cost to estimate:
the advice is asked for whenever the scene is assembled, and what collapsing
would occupy is asked for only when it is about to be shown to a sculptor who
is deciding.

#### Scenario: The offer appears when the engine advises it
- **WHEN** a field subtool has been worked until the engine advises collapsing it
- **THEN** the subtool panel offers to collapse it, and does not before

#### Scenario: Collapsing is the sculptor's decision
- **WHEN** the engine advises collapsing a subtool
- **THEN** nothing is collapsed until the sculptor asks for it

#### Scenario: The offer goes away once taken
- **WHEN** the user collapses the subtool
- **THEN** the layer reports itself collapsed and the offer is no longer made

### Requirement: Changing the active layer takes effect at once
Moving the sculpt target SHALL be atomic: when the command returns, every part
of the interface that describes the active subtool SHALL describe the new one.
The shelf, the active tool, the brush settings, the symmetry toggles and the
mask state SHALL all have followed before the next command is handled, so that
the **first** stroke after a switch is made with the new subtool's settings.

Commands that move the sculpt target SHALL include choosing a layer, adding one
and removing one. A new layer arrives active, and a removal hands the target to
whatever layer is left, which may hold a different representation.

A newly added layer SHALL NOT inherit brush settings from another
representation. Settings are held per tool and per representation, and the size
shown for the new layer SHALL be the size its next stroke uses.

A subtool that carries no mask SHALL report none as soon as it becomes active,
rather than continuing to report the mask of the subtool left behind. Moving
the sculpt target is not an edit, so this SHALL NOT depend on any later
document change to be observed.

A new armature layer SHALL start with its own symmetry rather than the previous
subtool's. The rig places its own reflected nodes, so the layer it is given is
created with its mirror off, and the interface SHALL show and use that mirror
from the moment the layer exists.

#### Scenario: The first stroke after a switch uses the new layer's brush
- **WHEN** a brush size is set on one subtool, another subtool is selected, and
  a stroke is made
- **THEN** the stroke is made with the newly selected subtool's brush size,
  tool and symmetry, not the previous subtool's

#### Scenario: A new layer does not inherit a brush size
- **WHEN** a grid layer is added while a field layer with its own brush size is
  active
- **THEN** the brush size reported for the new grid layer is the grid's own,
  and it is the size the next dab is made at

#### Scenario: Mask state follows the active subtool
- **WHEN** a mask is painted on one subtool and another subtool with no mask
  becomes active
- **THEN** the mask state reports no mask immediately, without any further
  command

#### Scenario: A new rig layer is not mirrored
- **WHEN** a rig is started while the active subtool has symmetry on
- **THEN** the new armature layer reports symmetry off, and the first ZSphere
  is placed unmirrored

### Requirement: A new layer declares its representation
Creating a layer SHALL offer the three representations — SDF, voxel and mesh
where a mesh source is at hand — and the resulting layer SHALL carry the
chosen representation's vocabulary from its first edit. The choice SHALL be
stated at creation rather than requiring a conversion afterwards.

#### Scenario: A voxel subtool is created directly
- **WHEN** the user adds a layer and chooses voxel
- **THEN** the new layer is voxel-backed and the voxel tools are available on
  it without a conversion step

#### Scenario: The default stays what it was
- **WHEN** the user adds a layer without engaging the choice
- **THEN** an SDF layer is created, as before

### Requirement: A subtool can be shown alone
The application SHALL offer a solo gesture on a layer: one action shows only
that layer, and releasing the solo restores the visibility each layer had
before it. Solo SHALL be a viewing convenience — it SHALL NOT change which
layer is active or add entries to the undo history.

#### Scenario: Solo isolates and restores
- **WHEN** three layers are visible, one is hidden, and the user solos a layer
  then releases the solo
- **THEN** during the solo only that layer is shown, and afterwards the three
  are visible and the fourth hidden, exactly as before

#### Scenario: Solo leaves history alone
- **WHEN** the user solos a layer, releases it, and undoes once
- **THEN** the undo applies to the last edit before the solo, not to
  visibility
