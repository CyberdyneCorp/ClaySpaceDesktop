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

A row SHALL name the entry point that executes for that pair, spelled in full.
A family abbreviated to one name plus suffixes — `begin/update/commit` — is a
name that cannot be looked up, and a row that names the kind of call rather than
the call is a row nothing can check. Where the same verb reaches the engine
through a resolved stroke for one tool and a single stamp for another, the rows
SHALL differ accordingly.

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
- **Pinçar** on SDF layers, through the magnify of the assembled surface at a
  negative strength.

A tool SHALL NOT be offered on a representation whose engine verb this
application does not reach, and a declared pair SHALL reach a distinct engine
call rather than falling through to a neighbouring one.

#### Scenario: A tool's label matches its verb
- **WHEN** the user selects Planar and applies it to a surface
- **THEN** the engine's flatten operation runs — cut-only on a field or a mesh,
  and two-sided on a grid, which is the verb the grid has

#### Scenario: Padrão and Inflar leave different marks on a field
- **WHEN** the same stroke is made on an SDF layer with Padrão and with Inflar
- **THEN** the two surfaces differ: one operation, two profiles, Padrão's a
  ridge that follows the falloff and Inflar's a broader and lower swell

#### Scenario: No orphan tools
- **WHEN** the tool registry is enumerated
- **THEN** every entry names the engine entry point it invokes, and none is unbound

#### Scenario: Every declared pair reaches its own verb
- **WHEN** each tool is applied on each representation its row declares
- **THEN** the edit lands, and no two tools on one representation resolve to the
  same engine call unless the table says they do

#### Scenario: A row names the call that runs
- **WHEN** a row names an entry point and the dispatch for that pair calls
  another
- **THEN** the row is wrong, whether or not the name it carries is a symbol the
  engine has

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
The interface SHALL expose the shaping parameters the engine's stroke engine and
brush parameters accept: an alpha curve, noise amount, **the angle each stamp is
turned about its own facing**, edge falloff, accumulation mode (buildup versus
clamped), stroke smoothing, and mirroring. Each SHALL map to a stroke preset or
brush parameter field, and SHALL NOT be presented if it has no engine
counterpart.

The stamp angle SHALL be set in degrees over a whole turn and SHALL wrap rather
than clamp, because an angle has no ends: a whole turn is none, and a value the
control cannot represent — a quantity that is not a number, or an infinity —
SHALL become no rotation rather than reaching the engine, which builds a rotation
basis out of it. Zero SHALL mean no rotation at all rather than a rotation by
zero, which is the value every stroke made before this control existed was made
with.

The angle SHALL be observable only where the footprint has something to orient. A
round brush with no stamp loaded looks the same at every angle by construction,
so the control SHALL be offered without being gated on an alpha being present:
gating it would make a setting appear and disappear as the sculptor changes
stamps, and the setting is held per tool.

The edge falloff SHALL be sent under the name the sculptor chose. Where the
engine's own reading of that name changes, the application SHALL follow the
engine rather than compensating for it behind the control, so that the name on
the dial and the curve on the surface stay the same thing.

#### Scenario: Buildup versus clamped differ observably
- **WHEN** the same stroke is applied twice over itself with accumulation enabled
  and again with it disabled
- **THEN** the accumulated pass deposits more than the clamped pass, matching the
  engine's buildup semantics

#### Scenario: Falloff selection reaches the engine
- **WHEN** the user selects an edge falloff
- **THEN** the corresponding falloff value is set in the brush parameters passed
  to the verb

#### Scenario: A turned stamp lands turned
- **WHEN** the same directional stamp is stroked along the same path twice, once
  upright and once at a quarter turn
- **THEN** the two strokes leave the material in different places

#### Scenario: A whole turn is none
- **WHEN** the stamp angle is set to a whole turn, or to a value that is not a
  representable angle
- **THEN** the setting reads as no rotation

#### Scenario: The name on the dial is the curve on the surface
- **WHEN** the engine's reading of a falloff name changes, and the user selects
  that falloff
- **THEN** the value sent is still the one that name stands for, rather than a
  neighbouring one chosen to reproduce the old curve

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

The gate SHALL be set on the stroke's own template, which is correct for every
stamp the stroke deposits because the engine measures a gate in **world space**:
the region it protects is where the mask was painted and stays there whatever
placement the gated item is then given. This is the opposite of the alpha rule,
where a deformer is resolved in the item's own frame and so cannot be carried by
a template — the two must not be reasoned about together.

A gate the engine refuses SHALL leave the stamp ungated rather than failing the
stroke. The engine refuses a gate that would protect nothing — an empty mask, or
one no cell of which reaches the threshold — and an ungated stamp is the correct
outcome in exactly that case.

Protection SHALL fade across a stated width rather than at a step. A gate is a
measured distance and not the painted mask, so painted softness is re-derived
from that width; a hard edge has no finite Lipschitz bound and nothing could
march it.

#### Scenario: A mask protects against a boolean
- **WHEN** a region is masked and a subtracting edit crosses it
- **THEN** the masked region is not cut

#### Scenario: An unmasked document is unaffected
- **WHEN** a subtracting stroke is made on a layer carrying no mask
- **THEN** the stroke is not refused and cuts as it always did

#### Scenario: Masking still keeps a brush from depositing
- **WHEN** a depositing stroke crosses a masked region wider than the brush
- **THEN** the masked region is not deposited into

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

The amount applied SHALL be the one the **command carries**. The menu fills it
in from the panel before it dispatches, which is where a menu entry gets to
spell out what it would do, and a caller at the agent door names its own. A
ViewModel that wrote the panel's amount over whatever arrived made `steps` a
parameter the door accepted and ignored.

An amount outside what the engine accepts SHALL be brought inside it, by the
same bounds the panel's own control uses, and SHALL be reported rather than
applied silently.

#### Scenario: An expansion reaches as far as the panel says
- **WHEN** the amount is set to four and Expandir is chosen
- **THEN** the frozen region grows further than it would at one

#### Scenario: An expansion reaches as far as a caller asked
- **WHEN** an expansion of four steps is asked for through the agent door while
  the panel stands at one
- **THEN** the region grows by four

#### Scenario: An amount the engine would refuse is brought in and reported
- **WHEN** an operation is asked for with no steps or with more than the engine
  takes
- **THEN** the operation is applied at the nearest amount that means something
  and the caller is told the amount was changed

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
It SHALL NOT offer the brushes that write vertex colour. It SHALL additionally
offer the eraser, which is not a fixed-topology brush and reaches no mesh
layer: a hierarchy stores detail in channels that can be taken back one at a
time, and a mesh has one surface with nothing stored beneath it.

Which tools reach a hierarchy SHALL be derived from the same declared table
every other representation is derived from, and the count SHALL be asserted
against the engine's own vocabulary so that a verb the engine gains is a
failing count rather than a silence. Both differences from the mesh column —
the two colour brushes that are absent and the one eraser that is present —
SHALL be asserted by name, so that a third difference appearing is a failure
rather than a shelf nobody looked at.

#### Scenario: The mesh brushes reach a hierarchy
- **WHEN** the tools offered on a hierarchy are listed
- **THEN** they are the tools offered on a mesh layer, less the two colour
  brushes and plus the eraser

#### Scenario: A tool the mesh sculptor does not have is not invented here
- **WHEN** a tool is offered on a hierarchy and not on a mesh layer
- **THEN** it is the eraser, which the representation earns rather than
  inherits, and no other

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

**A drag is the other exception, and it is a different one.** A drag's falloff
is not a coverage control at all: the grab is an inverse map, so the weight
decides where each cell in the ball takes its material *from*. Flattened to a
constant it translates the whole neighbourhood rigidly, which is a block being
shoved rather than clay being drawn. So a drag SHALL be written solid like
every other dab and SHALL keep a falloff that falls to the rim, and the two
SHALL NOT be conflated.

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

#### Scenario: A drag draws a bulge rather than shoving a block
- **WHEN** Mover drags material on a voxel layer and the surface is measured
  over the drag's centre and near the rim of its footprint
- **THEN** the centre rises materially further than the rim

#### Scenario: A drag as long as its own brush arrives short of the ask
- **WHEN** Mover drags material on a voxel layer by the brush's own radius
- **THEN** the surface rises by materially less than the distance asked for,
  because the pull tapers rather than carrying the ball rigidly

### Requirement: The capability table is checked against the engine that is linked
Every engine entry point the capability table names SHALL be a symbol the
build's own generated bindings declare, and every offered pair of tool and
representation SHALL reach an entry point its row names.

Both are properties of the table against the *engine*, and neither can be
asserted where the table lives: the domain links no engine and holds its verbs
as text. The list of declared entry points SHALL therefore be published by the
bridge, generated from the bindings rather than transcribed from the header, so
that what is checked is the ABI this build links.

What a stroke called SHALL be recorded where every fallible engine call already
passes, so the recorded name is the name that call would carry in its own error
and cannot fall out of step with it. The record SHALL NOT be compiled into a
build that does not ask for it.

A row MAY name more than one entry point, because more than one is reachable —
a field drag opens a transaction where it can and falls back where it cannot —
and the check is that the stroke reached one of them.

#### Scenario: An engine release renames a verb
- **WHEN** the engine pin moves and an entry point the table names is no longer
  declared
- **THEN** a test fails naming that entry point, rather than the row going on
  describing a call nobody makes

#### Scenario: A tool is rerouted to another entry point
- **WHEN** a tool's dispatch is changed to call an entry point its row does not
  name
- **THEN** a test fails naming the tool, the representation, what the row claims
  and what the stroke called

#### Scenario: A row that is right about the call
- **WHEN** every offered pair is stroked on a fixture of its representation
- **THEN** each one reaches an entry point its own row names

### Requirement: Each caveat about a representation is measured
Every caveat the application attaches to a tool on one representation SHALL
have a test measuring the difference it describes, and adding a caveat without
one SHALL NOT compile.

A caveat is shown to an artist as a fact about the engine's vocabulary. One
that is not measured is a sentence the interface tells because it reads well,
and the smoothing caveat on a hierarchy is exactly that today: it describes a
call no stroke opens.

A caveat that is not yet true SHALL be pinned by a test that fails when it
becomes true, so that closing the gap is announced rather than silent.

#### Scenario: A caveat is added without a measurement
- **WHEN** a caveat is added to the set the application can show
- **THEN** the test that names a measurement for each one stops compiling until
  it is given one

#### Scenario: A grid's flatten is two-sided
- **WHEN** the flatten is stroked across material a plane passes through on a
  grid
- **THEN** cells that were empty below the plane are filled as well as cells
  above it removed, where the scrape given the same plane fills none

#### Scenario: A hierarchy has no colour to write
- **WHEN** a colour brush is applied to a hierarchy layer
- **THEN** it is refused, where the same brush on the mesh the note sends an
  artist to is applied

### Requirement: A smooth on a hierarchy is made at the frequency chosen for it
A smooth stroke on a hierarchy SHALL be made through the engine's layered
stroke entry point carrying a stated frequency, and SHALL NOT be made as a
stamp carrying the smoothing brush, which is the plain Laplacian over positions
and removes detail it passes over.

The frequency SHALL be one of three: the form, the detail alone, or the form
with the detail re-applied unchanged. It SHALL default to the form with the
detail, which is what the tool's own caveat describes and the one of the three
that no other representation can offer.

The choice SHALL be offered on a hierarchy and on no other representation,
since the other three store one surface and therefore have one smooth, and it
SHALL be offered only while a smoothing tool is in hand.

Where a pass is active the smooth SHALL be written to that pass; where none is,
it SHALL be written to the form under them — the same rule every other stroke
on a hierarchy follows.

#### Scenario: A smooth that keeps the detail it passes over
- **WHEN** a sculptor smooths a region of a hierarchy carrying detail, at the
  form-with-detail frequency
- **THEN** the detail is left standing at its own height
- **AND** the form beneath it has moved

#### Scenario: A smooth that takes the detail off
- **WHEN** the same region is smoothed at the form frequency
- **THEN** the detail is lowered, as a plain Laplacian over it lowers it

#### Scenario: A representation with one smooth
- **WHEN** the smoothing tool is in hand on a field, a grid or a mesh layer
- **THEN** no frequency is offered

#### Scenario: Another tool on a hierarchy
- **WHEN** a tool that does not smooth is in hand on a hierarchy
- **THEN** no frequency is offered

#### Scenario: What a sculptor who has not chosen gets
- **WHEN** a smooth is made on a hierarchy and no frequency has been chosen in
  this session
- **THEN** it is made at the form with the detail carried through unchanged

### Requirement: Clearing a mask that freezes nothing costs nothing
Limpar is offered whether or not anything is frozen, because pressing it should
do the obvious nothing rather than be greyed out with a reason nobody needs.
That nothing SHALL cost nothing: where the active subtool freezes nothing, the
application SHALL NOT write to the mask, SHALL NOT send the frozen region to be
drawn again, and SHALL NOT record anything to take back.

It is not a refusal. The mask ends up exactly as the caller asked for it, so the
application SHALL say there was nothing to clear as a remark.

#### Scenario: Clearing an empty mask writes nothing
- **WHEN** Limpar is chosen on a subtool that freezes nothing
- **THEN** nothing is uploaded, nothing is added to the history, and the
  caller is told there was nothing to clear

### Requirement: Erasing on a hierarchy takes the selected pass toward zero
The application SHALL, when the eraser is used on a subdivision hierarchy, take
the selected pass's own detail toward zero, leaving the form beneath the passes
and every other pass exactly as they were.

It SHALL state on the tool, for a hierarchy alone, that this is what erasing
means there — the one label in the table over two different operations, since
on a grid the same tool clears the cells the brush covers and a hierarchy has
no cells to clear.

The erase SHALL be reported like any other stroke, so that a sculptor sees that
something happened even where the change is subtle, and one erase gesture SHALL
be one step in the edit history however many segments the drag arrived in.

#### Scenario: The pass goes and nothing else moves
- **WHEN** a sculptor erases over a region of a hierarchy with a pass selected
- **THEN** that pass's deposit is lowered in the region the brush covered
- **AND** the form beneath the passes and every other pass are unchanged

#### Scenario: One drag is one undo
- **WHEN** an erase gesture is made and then undone
- **THEN** the pass is back exactly as it was before the gesture, in one step

#### Scenario: The caveat is shown for the eraser on a hierarchy
- **WHEN** the eraser is shown against a hierarchy
- **THEN** its caveat says that erasing takes the selected pass toward zero

#### Scenario: The same tool on a grid carries no such caveat
- **WHEN** the eraser is shown against a voxel grid
- **THEN** no caveat is shown

### Requirement: Erasing is refused where the form is selected rather than a pass
The application SHALL refuse the eraser on a hierarchy whose selected row is
the form beneath the passes, and the refusal SHALL name the pass a sculptor has
to select. It SHALL NOT redirect the gesture to the form: walking the form's own
detail toward zero takes the whole surface back toward the pure subdivision,
which is a different operation at a scale an eraser does not suggest.

The refusal SHALL be a state of the layer rather than an absence from the
shelf, so the eraser stays visible and carries its reason, as every other tool
that exists for the active representation and cannot be used right now does.

The rule SHALL reach the eraser on a hierarchy and no other pair, since no
other representation has a row to select and no other verb means something
different in one row than in the other.

#### Scenario: The form is not a pass
- **WHEN** a sculptor selects the form's row on a hierarchy and erases
- **THEN** the stroke is refused, the refusal names a pass, and the surface
  does not move

#### Scenario: Selecting a pass is all it takes
- **WHEN** the sculptor then selects a pass and erases again
- **THEN** the stroke is made

#### Scenario: No other tool asks about a pass
- **WHEN** any other tool is used on any representation with no pass selected
- **THEN** it is not refused for want of one

### Requirement: A capability row carries what the call is, not only its name
Each column of the capability table SHALL be a typed binding carrying the
engine entry point, the intent the tool means by that call, the engine family
the call belongs to, and the fidelity with which it keeps the promise the
tool's label makes. No capability information SHALL be left in prose alone.

An entry point is a name. Whether a row is the representation's natural verb,
a strength the representation alone has, a useful stand-in, or several verbs
composed is what a sculptor is actually asking when they ask what a tool does
here — and stated as prose it can drive nothing and be held to nothing.

The fidelity of a binding SHALL match what the engine has been measured to do.
Where the engine documents one of two rows as the faithful implementation of an
intent and the other as its approximation, the table SHALL say which is which.

A binding MAY be a recipe: several engine verbs in a fixed order standing in
for one the engine does not have. A recipe SHALL be marked as one, so that a
composed tool is describable rather than absent.

The shelf, the availability refusal, the tool notes and the diagnostics report
SHALL all read this one table, and no other crate SHALL decide anything per
tool and representation.

#### Scenario: A field's Standard and its Inflate are ordered as measured
- **WHEN** the rows for Padrão and Inflar on an SDF layer are read
- **THEN** both name the relief operation, Inflar's binding is the native one
  and Padrão's is an approximation, and Padrão is the row carrying the caveat

#### Scenario: A composed tool is described rather than omitted
- **WHEN** a tool reaches a representation through several verbs rather than one
- **THEN** its row states them as a recipe, and the shelf, the refusal and the
  report describe it as one

#### Scenario: A second capability table is added elsewhere
- **WHEN** a View, a ViewModel, the engine adapter or the agent-facing crate
  decides something per tool and representation
- **THEN** the layering check fails, naming the file

### Requirement: The typed row's claims are checked
Every claim a typed binding makes SHALL be held by a test.

A tool SHALL mean one thing wherever it is offered: every binding of one tool
SHALL declare the same intent, because the shelf presents them as one button
with one tooltip and a column borrowed from a neighbouring verb is how that
button comes to mean two things.

A binding SHALL be filed under the family whose calls it names, where the
engine spells that family as a prefix.

A caveat SHALL NOT hang off a binding that claims to do exactly what its
label says. The caveat and the fidelity are two halves of one fact, and a
caveat on a faithful row is a sentence about nothing.

Two tools offered on one representation SHALL NOT have bindings identical in
every part, because then nothing distinguishes them but their labels. Where
that is nonetheless the truth, the pair SHALL be recorded with its reason, and
a recorded pair that has since come apart SHALL fail so the record is removed.

#### Scenario: A column is borrowed from a neighbouring verb
- **WHEN** one of a tool's bindings is changed to a call meaning something else
- **THEN** a test fails naming the tool and the two intents it would carry

#### Scenario: A caveat outlives the difference it described
- **WHEN** a binding's fidelity is corrected to the plain reading of its label
  while its caveat is left in place
- **THEN** a test fails naming the tool, the representation and the binding

#### Scenario: Two shelf entries collapse onto one binding
- **WHEN** two tools offered on one representation come to name the same call
  with the same intent, family and fidelity
- **THEN** a test fails unless the pair is recorded as one verb under two words,
  with the reason it still stands

### Requirement: A region-sampling verb is demonstrated against a surface it can act on
Four of the field verbs sample a region rather than stamp into it — the engine
adapter groups them as such — and each averages toward something the
neighbourhood already is.

A test that requires such a verb to move the surface SHALL measure it against a
surface that has something for it to do. A pristine sphere is the smoothest
thing there is, so requiring a smoothing verb to move one is requiring it to do
the job it exists *not* to do, and a fixture that passes on one is measuring
something other than the verb.

This SHALL NOT be met by lowering the threshold. The figure a verb has to clear
states what a sculptor would notice; a fixture that cannot produce it is the
part that is wrong.

#### Scenario: A smoothing verb is asked to smooth
- **WHEN** a region-sampling verb is tested for having any effect
- **THEN** it is applied to a surface carrying a feature it can flatten, and is
  required to flatten it by the same margin every other verb must move a
  surface by

#### Scenario: A stamping verb is unaffected
- **WHEN** a verb that displaces along a normal is tested for the same property
- **THEN** a resting surface is a sufficient fixture, and the threshold is
  unchanged

### Requirement: A curve's thickness is priced against the field
The application SHALL price the region a curve's tube would fill against what
this document's brick cache can hold, and SHALL refuse a thickness over that
budget with a sentence naming both the figure and the limit.

The price SHALL be paid before any control point's radius is written, so that a
refused thickness leaves the guide exactly as it was — every point at the
radius it had, and the tube unchanged.

A guide that has been refused a thickness SHALL remain a guide that can be
edited, have points removed, and be taken down.

#### Scenario: A thickness the field cannot hold
- **WHEN** a sculptor sets a curve radius whose tube would fill more of the
  field than the cache can hold
- **THEN** the thickness is refused, the refusal names what it would fill and
  what the document holds, and every control point keeps its own radius

#### Scenario: An ordinary thickness
- **WHEN** a sculptor sets a curve radius the field can carry
- **THEN** it is taken, and the points under the selection are given it

#### Scenario: A refused curve is still a curve
- **WHEN** a thickness has been refused
- **THEN** the guide can still have points removed and can still be taken down

### Requirement: Pinch on a field is a radial scale of the assembled surface
`Pinçar` on an SDF layer SHALL reach the engine's magnify of the assembled
surface at a **negative** strength, which gathers the region toward the dab's
centre. It SHALL NOT be bound to a stroke operation: relief and incise move the
surface along its own normal, and no shaping of that profile is a gather.

`Pinçar` SHALL be offered on a field. It was absent because the per-item
magnify gathers one contributor of a smooth union and leaves the rest; the
assembled-surface entry point resolves the region against every item it
reaches, which is what makes the tool possible there at all.

The region's radius SHALL come from the brush size and its easing from the drag
falloff, a magnify being a region deformation rather than a stamp. The
magnitude SHALL come from Intensidade, and the region SHALL be wider than the
brush, a gather having nothing to gather from within it otherwise.

A radial scale fixes its own centre: the point the region is centred on does
not move and the points nearest it barely do. The dab SHALL be left standing on
the surface, where the gesture's raycast put it, because a gather about a point
on the surface draws the material toward the stroke — which is what pinching is
— where a gather about a point sunk into the material deflates uniformly
instead.

The invert key SHALL spread: the material leaves the stroke instead of arriving
at it, which is the pair the grid's column already names for this tool.

A stroke SHALL lay one dab per step of the brush's spacing along the path,
rather than one per sample: the engine folds frames that share a centre, so a
pointer resting still would otherwise pile a gesture's worth of frames at one
place and keep only the last.

Under symmetry the gesture SHALL be applied once with the layer's mirror
pointed, rather than reflected and applied again. The engine reflects the
region into every image the layer emits and carries the strength across each
one untouched — a reflection of a radial scale is a radial scale of the same
strength — where a drag's displacement has to be mapped per image.

The frozen region SHALL be honoured by the stroke. The engine's descriptor
carries no gate, so samples the mask protects SHALL be dropped from the path
before any dab is placed.

The whole gesture SHALL be one step of the history the sculptor presses,
however many dabs it laid down.

#### Scenario: Pinch gathers the surface toward the stroke
- **WHEN** `Pinçar` is stroked across an SDF surface
- **THEN** the surface under the stroke stands proud and the rim of the
  region falls away, the material having moved toward the stroke

#### Scenario: Inverting the gather spreads
- **WHEN** `Pinçar` is stroked with the invert modifier held
- **THEN** the surface rises across the whole footprint and nowhere falls

#### Scenario: A magnify across a blend moves both contributors
- **WHEN** a magnify is stroked over the join of a form made of two
  smooth-unioned items
- **THEN** the surface moves on both sides of the blend rather than on one

#### Scenario: A gesture and its mirror are the same field
- **WHEN** a magnify is made with a symmetry axis on
- **THEN** the surface on the reflected side is left where the stroke's own
  side is

#### Scenario: One stroke is one undo
- **WHEN** a stroke long enough to lay down several dabs is made and then
  undone once
- **THEN** the whole stroke is taken back

### Requirement: A field's Standard says what its operation is faithful to
`Padrão` and `Inflar` on an SDF layer SHALL both remain bound to the relief
operation, differing in footprint and lift alone. Relief offsets the
accumulated field, so every point of the isosurface moves along the field's own
gradient; that is the **Inflate** frame, and the engine's own measurement of it
against frame-isolated references leaves nothing for a different operation to
improve on.

Because the same operation is therefore an approximation of Standard, `Padrão`
on a field SHALL carry a tool note stating the approximation and what decides
how far off it is: the spread of the normals under the stamp, which is a few
percent of the amplitude on a form smooth at the brush's scale and the whole
amplitude on a feature narrower than the stamp. The note SHALL offer the
remedy, which is a brush smaller than the feature.

The claim the note makes SHALL be held by a measurement rather than by the
prose, as every tool note's is.

#### Scenario: A thin feature takes the mark on its flanks
- **WHEN** one `Padrão` stamp is made on the top of a fin thinner than the
  brush, and on a sphere several times the brush
- **THEN** the fin grows sideways by nearly as much as its top rose, and the
  sphere does not, which is the divergence from a displacement along one
  averaged normal

### Requirement: A drag is seen while it is made
A gesture that **replays from its anchor** SHALL be sent to the model on every
pointer move, on every representation.

The threshold that holds a segment back exists because a *stamping* segment
costs a re-mesh of everything it touched, and sending one per pointer move
re-meshes the same neighbourhood repeatedly. That reasoning does not apply to a
replayed gesture: the whole drag is laid down from its anchor each time, so the
work is the same on the first segment and the fortieth, and waiting buys nothing
while costing exactly what a sculptor sees.

Whether a gesture replays SHALL therefore be asked **before** the representation
is asked, and not after it.

#### Scenario: A short field drag
- **WHEN** a field drag travels less than one stamp gap and the pointer is still
  down
- **THEN** the drag has already reached the model, and a live transaction opened
  for that gesture has been given segments to preview

#### Scenario: A short field stamping stroke
- **WHEN** a stamping stroke travels less than one stamp gap
- **THEN** nothing beyond the press's own dab has been sent, because that verb
  does not replay and every segment would cost a re-mesh

<!-- "Every Move drag is its own gesture" stood here. It asked for a name on
     every grab both Move doors write, and held its second scenario to the held
     drag because ClayCore v0.113.0's `clay_sdf_move_begin` did not read
     `gesture_id`. v0.116.0 carries that fix, so the rule is now part of "A drag
     costs the field the gesture, not the segments" in the living
     `sculpting-tools` spec, covering both doors. Keeping a second copy here
     would leave two texts for one rule, with nothing to say which the
     application obeys. -->

### Requirement: Mesh deformers honor the active painted mask
Mesh taper and twist SHALL leave fully masked vertices in place. Unmasked vertices SHALL receive the deformation. The lattice cage remains a whole form control point operation.

#### Scenario: Taper a masked mesh
- **WHEN** a mesh has a fully painted mask and taper is applied
- **THEN** its masked vertices SHALL keep their positions
