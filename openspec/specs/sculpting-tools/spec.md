# sculpting-tools Specification

## Purpose
Every verb a sculptor reaches for and the rules it obeys: the tools and the
engine verbs behind them, brush parameters and shaping, strokes and symmetry,
masks, cages, curves, and what a gesture shows while it is still being made.
## Requirements
### Requirement: Every tool maps to a documented engine verb
Each sculpting tool the interface presents SHALL correspond to a documented
ClayCore verb reached through the C ABI. The application SHALL NOT present a
tool that has no engine counterpart, and SHALL NOT bind a label to a verb whose
behavior differs from what the label states.

Where a tool applies SHALL be declared once, per tool and per representation,
in the table the shelf, the availability check, the diagnostics report and the
tests all read. Nothing else may decide where a tool applies.

Beyond the vocabulary already bound, the declared table SHALL include:

- **Mover** on voxel layers, through the grid's grab verb.
- **Planar** on voxel layers, through the grid's flatten verb, which is
  two-sided where the SDF and mesh sides are cut-only.
- **Vinco** on SDF layers, through the incise operation.
- **Argila** on SDF layers, through the relief operation with buildup
  accumulation.
- **Mover Topológico**, on SDF layers only, through the engine's topological
  move — a drag whose falloff is measured along the material rather than
  through space.

A tool SHALL NOT be offered on a representation whose engine verb this
application does not reach, and a declared pair SHALL reach a distinct engine
call rather than falling through to a neighbouring one.

#### Scenario: A tool's label matches its verb
- **WHEN** the user selects Planar and applies it to a surface
- **THEN** the engine's flatten operation runs — cut-only on a field or a mesh,
  and two-sided on a grid, which is the verb the grid has

#### Scenario: Padrão and Inflar leave different marks on a field
- **WHEN** the same stroke is made on an SDF layer with Padrão and with Inflar
- **THEN** the two surfaces differ: Padrão's mark is a ridge following the
  falloff, Inflar's a broader swelling of the footprint

#### Scenario: No orphan tools
- **WHEN** the tool registry is enumerated
- **THEN** every entry names the engine entry point it invokes, and none is unbound

#### Scenario: Every declared pair reaches its own verb
- **WHEN** each tool is applied on each representation its row declares
- **THEN** the edit lands, and no two tools on one representation resolve to the
  same engine call unless the table says they do

#### Scenario: Crease cuts a trough on a field
- **WHEN** Vinco is stroked across an SDF surface
- **THEN** a narrow trough is displaced into the accumulated surface through the
  incise operation, and no new primitive is added to the layer

#### Scenario: Crease inverted raises the ridge it would have cut
- **WHEN** Vinco is stroked with the invert modifier held on an SDF layer
- **THEN** the surface rises where the upright stroke would have cut, which is
  the operation the engine names as incise's inverse

#### Scenario: Clay builds up where a stroke crosses itself
- **WHEN** Argila is stroked twice over the same place on an SDF layer
- **THEN** the second pass adds to the first, and the same two passes with
  Camada do not, because Camada is the clamped-accumulation tool

#### Scenario: A topological drag does not reach across a gap
- **WHEN** Mover Topológico is dragged on a form whose two parts are close in
  space and far along the surface
- **THEN** only the part under the brush moves, where the Euclidean Mover at the
  same radius moves both

### Requirement: Brush strength, size and flow are directly controllable
The interface SHALL expose brush intensity, size and flow as always-visible controls whenever a sculpting tool is active. Each SHALL show its current value numerically and SHALL be adjustable both by dragging and by entering a value.

#### Scenario: Values persist per tool
- **WHEN** the user sets a size on one tool, switches to another, and switches back
- **THEN** the first tool's size is the value the user set, because settings are held per tool

#### Scenario: Size is expressed in the document's units
- **WHEN** the brush size is displayed
- **THEN** it is shown in the unit the document uses, so that a size means the same thing at any zoom level

### Requirement: Brush shaping controls are exposed
The interface SHALL expose the shaping parameters the engine's stroke engine and brush parameters accept: an alpha curve, noise amount, edge falloff, accumulation mode (buildup versus clamped), stroke smoothing, and mirroring. Each SHALL map to a stroke preset or brush parameter field, and SHALL NOT be presented if it has no engine counterpart.

#### Scenario: Buildup versus clamped differ observably
- **WHEN** the same stroke is applied twice over itself with accumulation enabled and again with it disabled
- **THEN** the accumulated pass deposits more than the clamped pass, matching the engine's buildup semantics

#### Scenario: Falloff selection reaches the engine
- **WHEN** the user selects an edge falloff
- **THEN** the corresponding falloff value is set in the brush parameters passed to the verb

### Requirement: Strokes are resolved by the engine's stroke engine
A drag across the surface SHALL be captured as stroke samples — position, pressure and timing — and resolved into edits by the engine's stroke engine, honoring arc-length spacing, pressure curves, jitter, taper and steady-stroke settings. The application SHALL NOT synthesize its own stamp spacing.

#### Scenario: Spacing follows arc length
- **WHEN** the user drags quickly across a region and slowly across another with the same settings
- **THEN** stamp spacing along the stroke is determined by distance travelled, not by the number of input samples received

#### Scenario: Pressure reaches the stroke
- **WHEN** a pressure-sensitive device reports varying pressure during a stroke
- **THEN** those pressure values are carried in the stroke samples handed to the engine

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

#### Scenario: A masked region resists every tool
- **WHEN** a region is fully masked and each available sculpting tool is applied over it
- **THEN** no tool alters that region

#### Scenario: Masks survive a resolution change
- **WHEN** the voxel resolution level changes on a layer carrying a mask
- **THEN** the mask still covers the same region of the model

### Requirement: A mask can be extruded into a new solid
The application SHALL expose mask extrude, producing a solid from the masked region, with outward, inward and centred options and a roundable rim, applied through the engine's mask-extrude entry point.

#### Scenario: Extract from a mask
- **WHEN** the user extrudes a painted mask outward with a rim radius
- **THEN** a new solid corresponding to the masked patch is added to the document, with the rim rounded as requested

### Requirement: The cut tool trims with a drawn shape
The application SHALL provide a cut tool that resolves a shape drawn on the view frame — rectangle, circle, polygon or lasso — into an engine cut, with keep-inner and keep-outer as the two outcomes and an optional rounding that bevels the cut walls.

#### Scenario: Keep-outer removes the enclosed region
- **WHEN** the user draws a rectangle over part of the model and chooses keep-outer
- **THEN** the enclosed material is removed and the rest is preserved

#### Scenario: An open curve closes against the frame
- **WHEN** the user draws an open curve and applies a trim
- **THEN** the curve is closed against the frame bounds rather than closed on itself, so the cut removes a side rather than a sliver

### Requirement: Voxel resolution levels are user-controllable
Where a layer is voxel-backed, the interface SHALL expose the voxel size, the stack of resolution levels, and which level is active, mapping to the engine's add, select and drop level operations. Adding a finer level SHALL NOT require re-authoring existing work.

#### Scenario: Block out coarse, detail fine
- **WHEN** the user adds a finer resolution level and sets it active
- **THEN** subsequent verbs edit the finer level and the coarser level's content is preserved

#### Scenario: A single-level grid is unaffected
- **WHEN** a layer carries only its original level
- **THEN** it behaves exactly as it did before multi-resolution controls were available

### Requirement: An edit that changed nothing is reported as such
Because many engine verbs can be valid calls that change nothing — a sub-cell drag, a stamp that misses every cell, a footprint over empty space — the application SHALL determine whether an edit changed anything by comparing the engine's change count across the call, and SHALL NOT treat a no-op as an error nor add it to the undo history.

#### Scenario: A sub-cell drag adds no history
- **WHEN** the user drags a voxel grab by less than one cell on every axis
- **THEN** nothing changes, no error is shown, and the undo history gains no entry

#### Scenario: A live edit is recorded
- **WHEN** an edit changes at least one cell
- **THEN** it is recorded in the undo history

### Requirement: Armatures are authored as a tree
The application SHALL expose armature authoring — a tree of spheres skinned by the engine's sphere-swept links and smooth union — allowing nodes to be added, moved, resized, reparented and removed, with the skin thickness controllable. Moving a parent node SHALL carry its subtree.

#### Scenario: Moving a parent carries the chain
- **WHEN** the user moves an armature node that has descendants
- **THEN** the descendants move with it and the skinned surface follows

#### Scenario: Armatures persist with the document
- **WHEN** a document containing an armature is saved and reopened
- **THEN** the armature tree is present and editable

### Requirement: The engine's combine operations and blend profiles are selectable
The application SHALL let a sculptor choose the combine operation an SDF edit
uses from those the engine provides, and the blend profile it is applied under.

The same vocabulary SHALL serve both kinds of edit: a stroke, where the choice
is made before the gesture and holds for it, and a placed object, where the
choice is a property of the object and stays editable for as long as the object
does. An operation SHALL mean the same thing in both.

This replaces a rule that spoke only of the edit about to be made. The
operations were always the engine's, and always applied to an item — a stroke's
item is created and left behind, and an object's stays addressable. Naming only
the first left the fourteen operations reachable exclusively through a gesture,
which is not how a boolean is used.

#### Scenario: An operation is chosen
- **WHEN** the user chooses a combine operation before making an edit
- **THEN** the edit is recorded with that operation

#### Scenario: A blend profile is chosen
- **WHEN** the user chooses a blend profile
- **THEN** edits made under it use that profile

#### Scenario: An object is placed with an operation
- **WHEN** the user places an object having chosen subtract
- **THEN** the object subtracts from what is under it, and the choice is
  recorded on the object rather than consumed

### Requirement: Alphas modulate a stamp
The application SHALL let a sculptor supply a scalar stamp pattern and apply it
through a brush on the representations where the engine accepts one. The
application SHALL state where an alpha is not available.

#### Scenario: An alpha is stamped
- **WHEN** the user applies a brush carrying an alpha
- **THEN** the surface shows the pattern under the brush's falloff

#### Scenario: An alpha where none is accepted
- **WHEN** the active representation accepts no alpha
- **THEN** the alpha control is unavailable and says so

### Requirement: Deformers act on a layer as authoring operations
The application SHALL offer the engine's deformers as operations on a layer,
distinct from brushes, with the parameters each takes and without requiring a
brush position.

#### Scenario: A deformer is applied
- **WHEN** the user applies a deformer to a layer with its parameters
- **THEN** the layer's form changes accordingly and the operation is one undo
  step

### Requirement: A voxel grid can be repaired before baking
The application SHALL offer the engine's pre-bake repair on a voxel layer:
reporting what is wrong, closing holes, and filling voids. The report SHALL be
shown before any repair is applied.

#### Scenario: A report precedes a repair
- **WHEN** the user opens repair on a voxel layer
- **THEN** the count of holes and voids is stated before anything is changed

#### Scenario: Holes are closed
- **WHEN** the user closes holes on a pierced shell
- **THEN** the report afterwards states fewer holes

### Requirement: Masks gate operations, not only brushes
The application SHALL apply a painted mask to any operation the engine can gate,
including combine operations, and not only to brush strokes.

#### Scenario: A mask protects against a boolean
- **WHEN** a region is masked and a subtracting edit crosses it
- **THEN** the masked region is not cut

### Requirement: Held keys substitute the verb and the sign for one gesture
The application SHALL let a sculptor smooth or take material away with the tool
already in hand, by holding a key for the length of one stroke, without
changing what the shelf has selected.

The keys SHALL be read at the press and held for the whole gesture, so a key
caught or released mid-drag does not change the verb under the sculptor's hand.

Inverting SHALL mean what it means on the active representation: a field turns
its combine operation over, a mesh negates its brush strength, and a grid
erases rather than deposits. An operation with no opposite SHALL be left as it
is rather than becoming a different verb.

#### Scenario: Shift smooths whatever is selected
- **WHEN** a stroke is begun with Shift held while a build-up tool is selected
- **THEN** every segment of that stroke smooths
- **AND** the next stroke, made without the key, builds up again

#### Scenario: The invert key digs on every representation
- **WHEN** the same stroke is made with the invert key held
- **THEN** a field is cut where it would have been raised
- **AND** a mesh vertex moves inward where it would have moved outward
- **AND** a grid's cells are cleared where they would have been set

### Requirement: A mask is painted and seen on every representation
The application SHALL offer the mask tool on SDF, voxel and mesh layers alike,
and painting one SHALL freeze a region rather than change the surface.

The frozen region SHALL be drawn over the surface it protects, on both the
brick-cache surface and the carried mesh and voxel layers.

A single key SHALL start mask painting and put the previous tool back.

#### Scenario: The mask tool freezes rather than deposits
- **WHEN** the mask tool is stroked across a voxel layer
- **THEN** a mask exists afterwards
- **AND** no material was added to the grid

#### Scenario: A painted mask is visible
- **WHEN** a mask is painted on the surface
- **THEN** the drawn surface is darker where the mask covers it
- **AND** clearing the mask returns the surface to what it was

#### Scenario: An edit beside a mask does not erase what is drawn
- **WHEN** a stroke re-meshes bricks that carried mask shading
- **THEN** the frozen region is still drawn afterwards

### Requirement: The mask operations take an amount the interface can set
The application SHALL let a sculptor set how far Expandir, Contrair and
Suavizar máscara reach, and what an extrusion's thickness, rim rounding and rim
smoothing are, and SHALL apply those amounts rather than fixed defaults.

Each menu entry SHALL show the amount it would apply.

#### Scenario: An expansion reaches as far as the panel says
- **WHEN** the amount is set to four and Expandir is chosen
- **THEN** the frozen region grows further than it would at one

#### Scenario: An extrusion is as thick as the panel says
- **WHEN** the thickness is set and the patch is extruded outward
- **THEN** the wall stands that far off the surface

#### Scenario: An operation with no amount is left alone
- **WHEN** an amount is set and Inverter is chosen
- **THEN** the operation carries no amount

### Requirement: A deformation cage bends the whole form
The application SHALL offer a lattice cage around the active layer, sized to
what that layer contains, with control points drawn in the viewport and
draggable directly.

The cage SHALL be worked in rather than applied per drag: the form follows when
the cage is applied, and the whole cage SHALL be one undo step however many
control points were dragged.

The cage SHALL be offered wherever the engine has a route for it, at the
resolution that route accepts, and refused readably where it has none.

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

### Requirement: A manipulator transforms a selection of control points
The application SHALL let a sculptor select more than one lattice control point
and move, turn or scale the selection with one manipulator.

The manipulator SHALL sit on the middle of the selection, and an axis handle
SHALL constrain its drag to that axis.

A drag SHALL be resolved from where it started rather than accumulated across
frames, and a scale SHALL never pass through zero.

#### Scenario: A whole face is moved at once
- **WHEN** the four control points of a cage's face are selected
- **AND** the manipulator's vertical axis is dragged up
- **THEN** all four move up together and none moves sideways

#### Scenario: A turn is about the selection's own middle
- **WHEN** a selection is turned a quarter about an axis
- **THEN** each point ends a quarter turn about the selection's middle
- **AND** nothing moves along the axis turned about

#### Scenario: A wandering drag lands where it ends
- **WHEN** a drag passes through an intermediate point before settling
- **THEN** the result is the same as a drag straight to where it settled

### Requirement: A mesh cage shows the bend while it is dragged
The application SHALL show what a lattice cage would do to a mesh layer while
its control points are being dragged, without committing to it.

The preview SHALL NOT compound across frames, SHALL leave the gesture one undo
step, and SHALL be taken back when the cage is abandoned.

#### Scenario: The form follows the cage
- **WHEN** a control point is dragged
- **THEN** the drawn surface has moved before anything is applied
- **AND** nothing has been recorded in the history

#### Scenario: A long drag lands where a short one does
- **WHEN** a drag arrives over twenty frames rather than one
- **THEN** the form ends in the same place

#### Scenario: Abandoning a cage abandons its preview
- **WHEN** a cage is dragged and then cancelled
- **THEN** the form is exactly as it was

### Requirement: Symmetry reaches every representation
The application SHALL apply the enabled symmetry axes to strokes on mesh and
voxel layers as well as on fields.

Each enabled axis SHALL add a full-strength copy of the stroke reflected
through that plane, and several axes SHALL give every combination of their
reflections. A reflected stroke's direction SHALL be reflected with it.

A symmetric stroke SHALL remain one undo step.

#### Scenario: The other side comes out the same
- **WHEN** a dab is made on a mesh layer with X symmetry on
- **THEN** the form stands the same at the dab and at its mirror
- **AND** the halves the other axes would reach are untouched

#### Scenario: Two axes give four
- **WHEN** a dab is made with X and Y symmetry on
- **THEN** all four quadrants carry the same form

#### Scenario: A mirrored drag is a reflection
- **WHEN** a drag is made outward along an axis with symmetry on that axis
- **THEN** the far side travels the opposite way by the same amount

### Requirement: A curve places a tube that can be edited afterwards
The application SHALL let a sculptor place a curve by putting control points
down, move those points afterwards, and sweep a tube along it.

Editing a control point SHALL replace the swept form rather than adding
another, and abandoning the curve SHALL take its form with it.

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

### Requirement: A gesture in progress is previewed without erasing itself
While a gesture is open, the model SHALL take back what its last segment did
only for verbs that are delivered again from their anchor on every segment. A
verb delivered as the samples that are new SHALL have its record *continued*
rather than replaced, so that a drag builds up as it is drawn.

Either way the gesture SHALL remain one undo, and taking it back SHALL put
every vertex where it was.

#### Scenario: A stamping drag builds up
- **WHEN** the user drags a stamping brush across a mesh in one gesture
- **THEN** every dab of the drag is on the surface when the gesture ends, not
  only the last

#### Scenario: A drag is one undo
- **WHEN** the user undoes a mesh drag once
- **THEN** the whole drag is taken back, however many segments drew it, and
  every vertex is where it was

### Requirement: A brush colour is chosen and reaches the colour brushes
The application SHALL hold one current brush colour, shared across tools rather
than stored per tool, together with a short list of recently used colours. The
interface SHALL offer a swatch to choose it, shown when the active tool writes
colour and hidden when it does not.

The colour SHALL reach the engine as the palette entry a voxel paint brush
deposits and as the colour a mesh paint stamp blends toward. Painting SHALL
change colour and SHALL NOT move the surface.

Choosing a colour already in a grid's palette SHALL reuse that entry rather
than adding a duplicate.

#### Scenario: Painting a grid changes what is drawn
- **WHEN** the user picks a colour and paints across a voxel layer
- **THEN** the painted cells carry that colour, the rendered image changes, and
  no vertex position moves

#### Scenario: A masked region keeps its colour
- **WHEN** a region of a voxel layer is masked and a paint stroke crosses it
- **THEN** the frozen cells keep the colour they had

#### Scenario: One paint gesture is one undo
- **WHEN** a paint stroke is made and then undone
- **THEN** the previous colours come back in one step, and redo puts the new
  ones back

#### Scenario: A painted colour survives the document
- **WHEN** a voxel layer is painted, saved, closed and opened again
- **THEN** the colours are the ones that were painted

#### Scenario: The swatch is offered only where it is read
- **WHEN** a tool that does not write colour is active
- **THEN** the colour swatch is not shown

### Requirement: A voxel drag accumulates below the cell size
Where a drag verb resamples occupancy per cell, the application SHALL
accumulate the gesture's displacement from its anchor and SHALL issue only the
part that has grown past a whole cell, rather than passing raw pointer deltas
that would round to no movement.

The displacement issued SHALL be measured from the gesture's anchor, so that a
slow drag and a fast one over the same path move the material equally far. One
drag SHALL be one undo entry.

Under symmetry both the drag's centre and its direction SHALL be reflected.

#### Scenario: A slow drag still moves the material
- **WHEN** a voxel layer is dragged with Mover in steps smaller than one cell
- **THEN** the material moves once the accumulated displacement passes a cell,
  and ends up where a single drag of the same total would have put it

#### Scenario: A drag differs from a smudge
- **WHEN** the same gesture is made with Mover and with Nudge on a voxel layer
- **THEN** Mover carries the body of the form and Nudge drags only its skin

#### Scenario: A mirrored drag pulls both sides outward
- **WHEN** X symmetry is on and a voxel layer is dragged away from the mirror
  plane
- **THEN** both sides move away from the plane, rather than both moving the same
  way in world space

### Requirement: A drag costs the field the gesture, not the segments
A drag with Move on a field SHALL cost the layer's field the same whatever
number of segments the gesture is delivered in. The same drag delivered more
finely SHALL NOT lengthen the layer's deformer chain, and SHALL NOT lower its
safe step scale, by more than measurement noise.

A whole drag SHALL be one history entry, however many segments drew it.

A segment SHALL carry the displacement measured from the gesture's **anchor**
rather than from the previous segment, so that a sequence of segments ends where
a single drag of the final displacement ends rather than at a composition of
them.

Every gesture SHALL be given a name no other gesture in the process has used,
and **both** Move doors — the live transaction and the held drag — SHALL send
that name with every grab they write. The engine folds a grab into the one
leading an item's chain only where both carry the same name.

Without the name the fold decides by centre and radius compared bit for bit,
which cannot tell a drag continuing from a second drag pressed at the same point
at the same size — and the fold *replaces*, because a drag re-sends its whole
displacement. So an unnamed second press at one anchor did not cost an extra
grab; it lost the first pull. The name is not saved: a reopened document's grabs
are unnamed and never match a new gesture.

Where the engine cannot hold a drag open on a layer, the application SHALL fall
back to applying it per segment, which is correct but costs more.

#### Scenario: The same drag cut more finely costs the same
- **WHEN** the user makes one drag delivered in four segments, and the same drag
  delivered in twelve
- **THEN** the layer's safe step scale is the same after both

#### Scenario: A drag is one action to undo
- **WHEN** the user completes a drag and undoes once
- **THEN** the whole drag is taken back, however many segments drew it

#### Scenario: Segments do not compose into a longer pull
- **WHEN** a drag is delivered as a sequence of segments
- **THEN** the surface ends where a single drag of the final displacement puts
  it

#### Scenario: A second drag from the same press keeps the first
- **WHEN** a Move drag is made, released, and a second is made from the same
  press point at the same brush size
- **THEN** the item carries one grab more than after the first drag, and the
  surface has moved further than the first drag left it

#### Scenario: Both doors name the gesture
- **WHEN** the same pair of drags is made once through the live transaction and
  once through the held drag
- **THEN** neither loses the first pull

### Requirement: A drag is shown while it is being made
The surface SHALL follow the pointer during a drag rather than appearing only
when the pointer is released.

While the gesture is open the document SHALL NOT carry any part of the drag: the
layer's field SHALL measure as it did before the gesture began, and the history
SHALL be unchanged. Where the application draws the preview by writing to the
layer and taking it back, it SHALL take it back within the same segment, so that
a segment leaves the history depth where it found it.

Abandoning a drag SHALL leave neither a mark on the document nor a preview on
the screen.

The committed drag SHALL land where the preview showed it.

#### Scenario: The surface follows the pointer
- **WHEN** the user drags with Move across the form
- **THEN** the surface the viewport draws changes before the drag ends

#### Scenario: The field is untouched while the pointer is down
- **WHEN** a drag is open and segments have been applied
- **THEN** the layer's safe step scale is what it was before the drag began, and
  the history is unchanged

#### Scenario: An abandoned drag leaves nothing behind
- **WHEN** the user abandons a drag in progress
- **THEN** the history is unchanged and the surface is drawn as it was before

#### Scenario: The drag lands where it was previewed
- **WHEN** a drag is committed
- **THEN** the surface stands where the preview showed it

### Requirement: A mirrored drag pulls each side once
A drag under a layer mirror SHALL pull each side by what one drag pulls, not by
what two do. Where the engine reflects a drag into every image the layer emits
of it, the application SHALL NOT reflect the gesture again.

#### Scenario: A mirrored live drag is not doubled
- **WHEN** the user drags with Move on a mirrored layer
- **THEN** the near side moves as far as it does on an unmirrored layer, and the
  far side moves with it

### Requirement: A region can be frozen by drawing round it
The application SHALL offer two drawn gestures for the mask alongside the brush
that paints it: one that traces a shape freehand, and one that drags a
rectangle square to the screen from corner to corner. Choosing the gesture SHALL
NOT change which tool is in hand: it is a property of the mask brush, and all
three gestures write the same mask.

The two drawn gestures SHALL differ only in how the pointer builds the shape. A
traced outline SHALL follow the pointer's path; a rectangle SHALL be the box
between the point pressed and the point the pointer is at now, however far the
pointer wandered between them, and SHALL be the same box whichever corner it was
started from. What either produces SHALL freeze the same region.

An outline drawn over the viewport SHALL freeze everything it encloses on the
**active subtool**, and nothing outside it. The region SHALL be the outline
swept straight along the view direction, so the surface behind the outline is
frozen with the surface in front of it.

The region SHALL be bounded by the active subtool's own extent. Where the
subtool states no extent, the gesture SHALL be refused in words rather than
freezing nothing silently.

An outline that encloses no area — a click, or a drag that went out and came
back along its own line — SHALL do nothing, and SHALL NOT put a refusal on the
screen.

An outline drawn away from the form SHALL leave the mask as it was, and SHALL
NOT be reported as a failure.

#### Scenario: The enclosed side resists and the rest does not
- **WHEN** the user draws an outline around one side of the form and then
  applies a brush inside it and outside it
- **THEN** the enclosed side is unchanged and the side outside the outline moves

#### Scenario: The far surface freezes with the near one
- **WHEN** the user draws an outline over the form and then applies a brush to
  the surface behind it
- **THEN** that surface is unchanged

#### Scenario: A concave outline freezes what was drawn, not its bounding box
- **WHEN** the user draws an outline with a concave side, such as a C, and then
  applies a brush inside the opening
- **THEN** the surface in the opening moves, because it was never enclosed

#### Scenario: A click with a drawn gesture in hand does nothing
- **WHEN** the user presses and releases without drawing
- **THEN** the mask is unchanged and no refusal is shown

#### Scenario: A dragged box freezes what a traced outline round the same region does
- **WHEN** the user drags a rectangle over one side of the form, and separately
  traces an outline round the same side
- **THEN** the two freeze the same region

### Requirement: The same gesture releases what it encloses
Drawing an outline with the modifier that inverts a stroke held SHALL release
what the outline encloses rather than freezing it, leaving the rest of the mask
alone.

Which of the two it will do SHALL be decided when the gesture begins and held
for its whole length, as a stroke's modifiers are, so a key taken up part-way
round cannot change what the outline means.

Which gesture is drawing SHALL likewise be settled when it begins: changing the
gesture with the pointer down SHALL abandon what has been drawn rather than
reinterpret it.

#### Scenario: Changing gesture mid-drag abandons the outline
- **WHEN** the user begins an outline and then chooses another gesture without
  releasing
- **THEN** the outline is abandoned and the mask is unchanged

#### Scenario: Releasing part of a mask keeps the rest
- **WHEN** the user freezes a region and then draws an outline inside it with
  the invert modifier held
- **THEN** the enclosed part is released, the rest stays frozen, and a brush can
  reach the released part again

### Requirement: The outline is drawn while it is being made
The viewport SHALL trace the outline as the pointer draws it and SHALL
distinguish an outline that will freeze from one that will release.

Where the shape closes itself across a gap the sculptor can see — a traced
outline, whose ends need not meet — the viewport SHALL show the edge that will
close it, and SHALL show it as less certain than the edges actually drawn. A
rectangle has no such gap and SHALL be drawn as four equal edges.

The outline SHALL be taken down when the gesture ends, whether it was applied,
refused, or abandoned.

A gesture that begins off the form SHALL still be a gesture: pressing beside the
form with a drawn gesture in hand SHALL begin an outline rather than turning the
camera, since an outline is drawn *around* a region.

While a drawn gesture is in hand the brush ring SHALL NOT be drawn. A ring says
the next press leaves a stroke where it sits, and with a drawn gesture in hand
the next press draws a line on the screen instead — which the surface has no
footprint for.

#### Scenario: The line follows the pointer
- **WHEN** the user is part way through tracing an outline
- **THEN** the viewport shows the line drawn so far and where it will close

#### Scenario: A rectangle is drawn as a box
- **WHEN** the user is part way through dragging a rectangle
- **THEN** the viewport shows the box between the corner pressed and the pointer

#### Scenario: An abandoned outline leaves nothing behind
- **WHEN** the user abandons an outline in progress
- **THEN** no line is left on the viewport and the mask is unchanged

#### Scenario: The brush ring is off while a shape is being drawn
- **WHEN** the user chooses a drawn gesture and moves the pointer over the form
- **THEN** no brush ring is drawn, because no press there would leave a stroke

### Requirement: A whole lasso is one edit
A lasso SHALL reach the mask as a single recorded edit: one undo takes the whole
region back, and one redo puts it back, however many cells it covered.

The viewport SHALL re-sample the frozen region when a lasso lands, since a lasso
moves no clay and nothing else would prompt it.

The gesture SHALL be bounded in cost: a region too large to write in a bounded
time SHALL be refused in words that say what to do instead, rather than being
started and appearing to hang.

#### Scenario: One undo takes a lasso back
- **WHEN** the user freezes a region with a lasso and undoes once
- **THEN** nothing is frozen

#### Scenario: The frozen region is drawn straight away
- **WHEN** a lasso lands
- **THEN** the viewport draws the newly frozen region without any further edit

#### Scenario: A region too large to freeze at once says so
- **WHEN** the user draws an outline around the whole of a subtool large enough
  that freezing it would take an unbounded time
- **THEN** the gesture is refused with a reason, and the mask is unchanged

### Requirement: A hierarchy is sculpted with the mesh vocabulary less its colour
The application SHALL offer, on a layer holding a subdivision hierarchy, the
fixed-topology brushes it offers on a mesh layer, together with the mask brush.
It SHALL NOT offer the brushes that write vertex colour. Which tools reach a
hierarchy SHALL be derived from the same declared table every other
representation is derived from, and the count SHALL be asserted against the
engine's own vocabulary so that a verb the engine gains is a failing count
rather than a silence.

#### Scenario: The mesh brushes reach a hierarchy
- **WHEN** the tools offered on a hierarchy are listed
- **THEN** they are the tools offered on a mesh layer, less the two colour
  brushes

#### Scenario: A tool the mesh sculptor does not have is not invented here
- **WHEN** a tool is offered on a hierarchy
- **THEN** it is also offered on a mesh layer

#### Scenario: The mask is the same call wherever it is painted
- **WHEN** the mask brush is used on any representation
- **THEN** it invokes the same engine verb, because a mask belongs to none of
  them

### Requirement: A colour brush on a hierarchy is refused with the reason
The application SHALL state, when a colour brush is asked for on a hierarchy,
that a hierarchy stores where a vertex went rather than what colour it is, in
addition to naming the representations where the brush does apply.

#### Scenario: The refusal says more than where else it works
- **WHEN** a colour brush is selected against a hierarchy
- **THEN** the refusal names both the representations that do carry colour and
  the reason this one does not

#### Scenario: Only this absence carries a reason
- **WHEN** any other tool is absent from any other representation
- **THEN** the refusal names where the tool applies and nothing further

### Requirement: A smooth on a hierarchy states that it picks a frequency
The application SHALL state, on the smoothing tool for a hierarchy alone, that
a smooth there acts on the form, on the detail alone, or on the form with the
detail carried through unchanged.

#### Scenario: The caveat is shown for the smooth on a hierarchy
- **WHEN** the smoothing tool is shown against a hierarchy
- **THEN** its caveat says the smooth picks a frequency

#### Scenario: The same tool on a mesh carries no such caveat
- **WHEN** the smoothing tool is shown against a mesh layer
- **THEN** no caveat is shown

### Requirement: A hierarchy with no cage yet says what it is waiting for
The application SHALL refuse the tools on a hierarchy row whose cage has not
arrived, naming the cage rather than naming geometry, so that the refusal sends
a sculptor to the crossing that builds one.

#### Scenario: A row before its cage
- **WHEN** a tool is asked for on a hierarchy carrying no geometry
- **THEN** it is refused, and the refusal names a cage

#### Scenario: A mesh row before its triangles says something different
- **WHEN** a tool is asked for on a mesh layer carrying no geometry
- **THEN** the refusal names a mesh

### Requirement: A live gesture is shown while it is being made, over the whole scene
A stroke with a region tool on a field SHALL be shown on the surface as it is
made, rather than only when the pointer is released, wherever the engine can
hold the gesture open.

While such a gesture is open the document SHALL NOT change: no items, no
deformers and no history entries until the gesture is laid down. This SHALL
hold for everything the preview does, including reading the rest of the
document in order to draw it.

The surface SHALL come to rest where the preview showed it. The preview and
the result need not be the same computation — the preview may be produced by
machinery the final edit does not use — but the difference SHALL be smaller
than a sculptor can see, and SHALL be measured rather than assumed.

Laying the gesture down SHALL NOT degrade the rest of the subtool. In
particular the application SHALL NOT consolidate a whole layer as a side
effect of a stroke that touched part of it.

**A preview SHALL show the rest of the scene beside the layer it previews.**
Where other field subtools are visible, the application SHALL compose them into
what it draws, and the composition SHALL be the engine's own union rather than
an approximation of it. It SHALL NOT reach that composition by editing the
document — hiding layers around the gesture and showing them again is an edit,
and a gesture that is holding a layer refuses one.

A gesture that cannot be shown live SHALL fall back to being held whole and
applied when the pointer is released, which is correct but not live. The
application SHALL NOT draw a preview it cannot compose correctly.

The previewed surface SHALL be meshed by the engine from samples the engine
computed. The application SHALL NOT interpolate, resample or otherwise decide
where the previewed surface lies.

#### Scenario: The surface moves while the pointer is down
- **WHEN** the user drags the smoothing brush across the form
- **THEN** the surface the viewport draws changes before the stroke ends

#### Scenario: Nothing is written down until the stroke ends
- **WHEN** a live gesture is open and dabs have been applied
- **THEN** the document's history is unchanged, and abandoning the gesture
  leaves the surface exactly as it was

#### Scenario: The result lands where the preview showed it
- **WHEN** a live gesture is laid down
- **THEN** the surface stands where the preview showed it, within a stated
  tolerance, rather than visibly moving when the gesture ends

#### Scenario: A stroke does not re-bake the whole subtool
- **WHEN** the user smooths part of a subtool
- **THEN** the rest of it is left as it was, at the resolution it had

#### Scenario: A second field subtool is still drawn
- **WHEN** a second field subtool is visible and the user smooths the first
- **THEN** the gesture is shown live, and the second subtool is on screen for
  the whole of it

#### Scenario: Reading the rest of the scene does not spoil the gesture
- **WHEN** a live gesture composes the rest of the document into its preview,
  on every segment
- **THEN** the history depth does not move while the gesture is open, and the
  gesture still commits when the pointer is released

#### Scenario: A hidden or empty subtool is neither drawn nor a refusal
- **WHEN** the other field subtool in the document is hidden, or has nothing
  in it
- **THEN** the gesture opens, and what is drawn is what would be drawn with
  that subtool absent

#### Scenario: A live gesture is one action to undo
- **WHEN** a live gesture is committed and the user undoes once
- **THEN** the whole gesture is taken back, however many dabs drew it

### Requirement: A pull's taper is fixed when a point is placed
The Snake Hook authors a curve whose control points carry a radius each, and
the radius SHALL be a function of the distance travelled from the anchor.

It SHALL NOT be a function of the stroke's length. A taper measured across the
point index is renormalised by every additional sample, which changes the
radius of points the sculptor has already placed and is no longer touching —
measured, a point at index 5 thickened by 82% over one forty-sample pull.

This is a requirement about the *field*, not only about appearance. A radius
that can change behind the cursor makes the whole curve's field change on every
segment, which makes the whole curve's bricks correctly dirty and the pull
quadratic in its own length.

A pull longer than the taper's span SHALL hold its tip thickness rather than
re-thinning what is behind it.

#### Scenario: A pull is extended past a point already placed
- **WHEN** a Snake Hook pull is continued beyond a control point already sent
  to the engine
- **THEN** the field around that earlier point is unchanged, within the curve
  fitting tolerance

#### Scenario: A pull still reads as a tendril
- **WHEN** a pull is drawn out to full length
- **THEN** it is thicker at its root than near its tip

### Requirement: A live segment re-evaluates only what it changed
A segment of a gesture that grows an existing item SHALL dirty the region its
new samples changed, together with every image the layer mirror places that
region at, rather than the whole item's bound.

Each image SHALL be marked as its own region. A single box containing an image
and its reflection spans the untouched form between them.

Where the changed region cannot be named — the item did not grow, or this is
its first delivery — the item's own bound SHALL be used instead. Correct is the
direction to fail in, because the engine computes that bound in world space
from the document and it is right under every transform.

#### Scenario: A mirrored pull is drawn
- **WHEN** a segment of a pull is applied with a symmetry axis enabled
- **THEN** the reflection is dirtied in the same segment, and appears while the
  stroke is being drawn rather than when it ends

#### Scenario: A segment reports its own cost
- **WHEN** a segment grows a pull
- **THEN** the edit's dirty-brick count is what that segment dirtied, so that a
  profile of the stroke measures the segment rather than a constant

### Requirement: A curve's guide is the line its tube follows
The viewport SHALL draw a placed curve's guide as the tessellation of its join,
not as the chords between its control points.

The two are the same line only for `Corners`. For a join that curves, the
chords cut the corners the tube rounds, and the guide is the only line the
sculptor can see — the tube conceals its own centre.

The tessellation SHALL be verified against the swept field rather than assumed
to match it. No ABI call returns a swept guide's tessellation, so the interface
computes its own, and agreement is a measurement: a sample on the guide reads
about minus the tube's radius, which is what the centre of a tube that thick
reads.

Membership SHALL NOT be accepted as that measurement. "Every sample is inside
the tube" is true of the control polygon as well, on any bend gentle enough
relative to the radius, and so cannot distinguish the right line from the wrong
one.

#### Scenario: A curve is drawn with a join that bends
- **WHEN** a curve using `Through` or `Rounded` is placed
- **THEN** the drawn guide passes through the tube's centre along its length,
  and not along the straight chords between the control points

### Requirement: A curve's guide is legible against its own tube
While a curve is being placed or edited, the surface SHALL be drawn ghosted, as
it is while a deformation cage is up.

The scaffold pass fades whatever the sculpt stands in front of. A cage's
control points are half behind the form; a curve's guide is *entirely* inside
the tube it describes, so without this the whole line is drawn faded.

Legibility SHALL be measured as the contrast the guide's pixels carry against
the surface behind them, not as the count of pixels it changes. The pass fades
the line rather than hiding it, so a count is cleared whether or not the fix is
present.

#### Scenario: A tube is swept along a curve still being edited
- **WHEN** the guide runs inside the tube it has swept
- **THEN** it reads against the tube rather than being faded into it

### Requirement: A curve can be refined between its points
A double-click on a curve's guide SHALL insert a control point that splits the
span under the pointer, selecting the new point.

Appending is what a click on empty space does. A curve that can only grow at
its end cannot be refined in the middle, which is where a tube usually needs
another point.

The span SHALL be resolved from the guide rather than the control points: the
second press of a double lands within a handle's reach of the point the first
one selected, so testing the points first would turn every double-click into a
re-selection.

#### Scenario: A sculptor double-clicks a bend
- **WHEN** a double-click lands on the guide between two control points
- **THEN** a control point is inserted between exactly those two, the points
  before and after it keep their places, and the new one is the selection

### Requirement: A curve can be drawn as well as clicked
A drag beginning where there is no control point and no guide SHALL lay a chain
of control points along the pointer's path, and a click in the same place SHALL
lay exactly one.

The two SHALL be told apart by distance travelled rather than by a mode. A press
that never moves never reaches the spacing, so one path serves both.

Spacing SHALL be measured in tube-widths. A point per frame is a curve that
cannot be edited afterwards, and being able to go back to it is what separates
this tool from a brush.

A freehand stroke SHALL stay on the plane its first point chose, and SHALL NOT
re-pick the surface per point: by the second point the tube the stroke is
drawing is under the pointer, so a ray cast at the surface lands on the stroke's
own output.

#### Scenario: A sculptor drags to draw a tube
- **WHEN** the pointer is dragged from a place holding neither a control point
  nor the guide
- **THEN** control points are laid along the path at tube-width spacing, and
  the tube follows them

#### Scenario: A sculptor clicks without moving
- **WHEN** a press begins and ends without travelling
- **THEN** exactly one control point is placed

### Requirement: A press on the guide does not extend the curve
A press landing on the guide, where no control point is under the pointer,
SHALL be consumed without adding a control point.

Appending there adds a point at the *end* of the curve — nowhere near the
pointer — and selects it, so the line appears to jump to a place nobody
clicked. It also makes a double-click on the guide unreachable, because the
first press of the double has already appended before the second can be read.

The order in which a press is resolved SHALL be decidable without a viewport: a
control point first, then a double on the guide, then the guide alone, then
empty space.

#### Scenario: A sculptor clicks the line once
- **WHEN** a single press lands on the guide away from any control point
- **THEN** the curve is unchanged and no point is added

#### Scenario: A sculptor double-clicks a control point
- **WHEN** a double-click lands on an existing control point
- **THEN** the point is taken in hand rather than a second one being inserted
  coincident with it

### Requirement: Laying a control point costs the end it added
An appended control point SHALL dirty the region the new end changed, together
with every image the layer mirror places it at, rather than the swept node's
own bound.

A change that is not an append — a point moved or removed, a radius, join or
profile changed — SHALL use the node's own bound, since any of those can move
the whole tube.

The regions SHALL be marked separately rather than unioned into one box, which
would span the untouched form between an image and its reflection.

This SHALL be held by a test that detects **staleness**, not only cost: a brick
the append changed but the region did not name keeps its old value and nothing
reports it. Measuring the surface, refilling everything, and measuring again is
what distinguishes a region that named enough from one that merely named less.

#### Scenario: A curve is drawn freehand
- **WHEN** control points are appended one after another
- **THEN** each costs the end it added rather than the whole tube

#### Scenario: A curve laid point by point is compared with one refilled whole
- **WHEN** the layer is refilled from scratch after an incremental build
- **THEN** the surface is unchanged

### Requirement: The brush ring is not drawn while a curve is placed
The brush ring SHALL NOT be drawn while a curve is being placed or edited.

A ring under the pointer states that the next press leaves a stroke there. While
a curve is up a press puts a control point down, takes hold of one, draws a
chain of them, or is spent on the guide — none of which is a dab, so the ring
would be promising something no press there can deliver.

This SHALL be a clause of the rule that answers whether the ring is drawn,
alongside the whole-subtool manipulator, the deformation cage and the mask's
drawn gestures, rather than a condition at the call site: they are the same
question and a fourth answer kept somewhere else is how the third one came to
be missed.

#### Scenario: A curve is active
- **WHEN** the pointer is over the form with a curve being placed
- **THEN** no brush ring is drawn

### Requirement: Dragging a control point costs what the drag disturbed
A drag SHALL dirty the neighbourhood of the points that moved — both where they
were and where they now are — rather than the swept node's whole bound.

Both, because the field changed in both places: refilling only the destination
leaves the shape the point left standing on the surface with nothing to report
it.

The regions SHALL be one box per affected point rather than one box around the
range. A bent tube's enclosing box is mostly air.

The margin SHALL be measured against a **rendered** surface, not a pick. A pick
is answered from a path that stays correct whether or not the brick cache was
refilled, so it cannot detect a region that is too small — and the value that
merely passes a pick may sit below a real cliff.

Margins on different paths SHALL NOT be made to agree for tidiness. Each is
measured against its own cliff and its own cost, and the answers differ.

#### Scenario: A control point is dragged clear of where it started
- **WHEN** the surface is rendered after the drag, and again after every brick
  the tube reaches is refilled
- **THEN** the two pictures agree

#### Scenario: A margin is chosen
- **WHEN** a refill region's margin is set
- **THEN** the value sits above a cliff found by making the guard fail, and its
  cost on the path that runs most often is measured before it is widened

### Requirement: A grid dab writes solid material
A dab on a voxel layer SHALL write its whole footprint, and intensity SHALL
scale the footprint's radius rather than the fraction of cells written.

Occupancy is binary, so a weight between 0 and 1 can only be spent as scattered
coverage. Spending it that way left the middle of a stroke porous, and with a
seed fixed for every dab the skipped cells were the same ones each time, so no
amount of overlap closed them.

An alpha carve is the exception: the stamp's own greys have nowhere but
coverage to live, so it still dithers, with a seed that differs per dab.

#### Scenario: A stroke at the shelf's defaults
- **WHEN** a Padrão stroke is made on a grid at the default intensity and
  falloff
- **THEN** no cell in the core of its footprint is left empty

#### Scenario: A lighter brush
- **WHEN** the same stroke is made at a low intensity and at full intensity
- **THEN** the lighter stroke writes materially fewer cells than the full one,
  and both write their core solid

#### Scenario: A drag on a grid
- **WHEN** Mover drags material on a voxel layer
- **THEN** the cells it carries are the whole footprint rather than a scattered
  subset of it

