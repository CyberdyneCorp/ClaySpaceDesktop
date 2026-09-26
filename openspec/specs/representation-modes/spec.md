# representation-modes Specification

## Purpose
What the active layer's representation is, where a sculptor can see it, and how
the shell's tools and panels follow it — so that each of field, voxel, mesh and
subdivision hierarchy offers its own vocabulary rather than one list with
entries greyed out.
## Requirements
### Requirement: The active representation is visible without inspection
The application SHALL show the active layer's representation above the
viewport, as one card per representation standing together — so that a sculptor
sees not only what the active layer is but what the alternatives are — and in
the layer stack. Neither SHALL require opening a panel. The cards SHALL be
distinguishable from each other by more than colour alone: each SHALL carry an
icon of a distinct shape *and* its name.

A subdivision hierarchy is one of the representations the bar draws, so the bar
carries four cards rather than three. Only two of the four can be created from
nothing — a hierarchy is built from a cage and a mesh is carried in — and the
bar is a statement of what a layer *is*, not an offer to make one, so it shows
every representation the application knows about rather than only the creatable
ones.

The active card SHALL be distinguished by surface tone and an accent rail, in
the same grammar the active layer row uses, so that the state survives the hue
being removed.

Every word the bar draws SHALL come from the interface's own table. It SHALL
NOT draw a representation's engine label, which reads the same in every
language.

#### Scenario: The representation is on screen
- **WHEN** a layer is active
- **THEN** its representation is lit among the cards above the viewport, and named beside the layer in the stack

#### Scenario: The alternatives are on screen too
- **WHEN** a layer is active
- **THEN** the other representations are shown beside it, named and explained, rather than being invisible until a panel is opened

#### Scenario: Switching layers changes what is shown
- **WHEN** the user makes a layer of a different representation active
- **THEN** the lit card changes to match, and so does the tag in the stack

#### Scenario: The bar reads in the interface's language
- **WHEN** the interface is set to any supported locale
- **THEN** every name and phrase in the bar is in that language

#### Scenario: A subdivision hierarchy is one of them
- **WHEN** the set of representations the application knows about is listed
- **THEN** a subdivision hierarchy is among them, with a name, a phrase saying
  what it is, an icon of its own shape and a short tag, in every language the
  interface offers

### Requirement: The tool shelf offers the verbs the active representation has
The application SHALL present, for the active layer, the tools that exist for
that representation. A tool that has no verb on the active representation SHALL
NOT be offered. Whether a tool exists for a representation SHALL be derived from
one declared table rather than from a rule written per tool.

The shelf MAY additionally let a sculptor browse another representation's
vocabulary, or a shortlist of their own, on request. While browsing either, a
tool the active layer has no verb for SHALL be shown as unavailable — dimmed,
with the reason on hover — and SHALL NOT be selectable. Browsing SHALL NOT be
the default: with nothing chosen the shelf SHALL show exactly the tools the
active layer can be sculpted with.

A shortlist SHALL be the user's own and SHALL span every representation, since
its purpose is finding a brush again rather than describing the active layer.
Where the shortlist is chosen and is empty, the shelf SHALL say how to add to
it.

Which set the shelf is showing is interface state. It SHALL emit no command and
SHALL NOT survive the application closing. The shortlist itself SHALL survive,
because it is a preference rather than a view.

#### Scenario: The shelf follows the active layer
- **WHEN** the user makes a voxel layer active and then a mesh layer active
- **THEN** the shelf's contents change to the verbs each representation has

#### Scenario: A tool with no verb here is absent
- **WHEN** a representation has no verb for a tool, and neither another representation nor the shortlist is being browsed
- **THEN** the tool is not shown in the shelf for that layer

#### Scenario: Browsing answers what another representation has
- **WHEN** the user asks to see another representation's tools
- **THEN** that representation's tools are listed

#### Scenario: A browsed tool cannot be picked
- **WHEN** the user clicks a tool the active layer has no verb for while browsing
- **THEN** the active tool does not change, and the reason is available on hover

#### Scenario: A shortlist spans the representations
- **WHEN** the user stars a brush that the active layer has no verb for and then browses the shortlist
- **THEN** that brush is listed, dimmed, and cannot be picked

#### Scenario: An empty shortlist explains itself
- **WHEN** the shortlist is chosen and nothing has been starred
- **THEN** the shelf says so, and says where the gesture is

#### Scenario: Browsing is not remembered
- **WHEN** the application is closed while another representation is being browsed and opened again
- **THEN** the shelf shows the active layer's own tools

#### Scenario: The active tool survives where it can
- **WHEN** the active tool exists on the newly active layer's representation
- **THEN** it stays active rather than being reset

#### Scenario: The active tool is replaced where it cannot
- **WHEN** the active tool does not exist on the newly active layer's
  representation
- **THEN** the application selects one that does and states that it changed

### Requirement: A tool unavailable for a reason other than its representation says so
The application SHALL continue to disable, with a stated reason, a tool that
exists for the active representation but cannot be used right now — a protected
layer, a hidden layer, or a missing prerequisite such as a mesh without a colour
attribute.

#### Scenario: A protected layer disables its tools
- **WHEN** the active layer is protected and a tool that exists for its
  representation is offered
- **THEN** the tool is disabled and names the protection as the reason

#### Scenario: A missing prerequisite is named
- **WHEN** a colour brush is offered on a mesh layer carrying no colour
  attribute
- **THEN** the tool is disabled and states that the mesh has no colour to paint

### Requirement: Brush settings are held per tool and per representation
The application SHALL remember brush settings for each tool separately on each
representation, so that returning to a tool on a layer returns the settings it
had there.

#### Scenario: Settings return with the layer
- **WHEN** the user sets a size on a tool on a voxel layer, works on an SDF
  layer, and returns to the voxel layer
- **THEN** the tool has the size it had on the voxel layer

### Requirement: The manipulator turns a selection in the screen's frame as well as the world's
The application SHALL offer, while the manipulator is set to rotate, a ring that
turns the selection about the axis facing the camera, in addition to the three
world-axis rings. The axis SHALL be fixed when the drag begins, so that moving
the camera during a drag does not alter the rotation.

#### Scenario: The outer ring turns in the screen plane
- **WHEN** the user drags the outer ring
- **THEN** the selection turns about the direction from it to the camera, and
  points along that direction do not move

#### Scenario: Moving the camera mid-drag does not twist the selection
- **WHEN** the camera moves while an outer-ring drag is in progress
- **THEN** the rotation continues about the axis the drag began with

### Requirement: A rotation can be snapped to whole increments
The application SHALL round a rotation to increments of 15 degrees while a
modifier is held, and SHALL read that modifier for as long as the drag lasts
rather than only when it begins. Snapping SHALL apply to rotation alone.

#### Scenario: A snapped turn lands on an increment
- **WHEN** the user turns the manipulator by an angle that is not a multiple of
  15 degrees, with the modifier held
- **THEN** the selection is turned by the nearest multiple of 15 degrees

#### Scenario: The modifier can be taken up part-way through a turn
- **WHEN** the user begins a turn without the modifier and presses it before
  releasing
- **THEN** the turn snaps from that point on

#### Scenario: Moving and scaling are unaffected
- **WHEN** the modifier is held during a move or a scale
- **THEN** the result is the same as without it

### Requirement: The inspector answers what is being sculpted
The right region SHALL carry one section describing the active layer's
representation, in a fixed position. Its contents SHALL change with the
representation; its position SHALL NOT, so that the sections around it stay
where a sculptor left them.

The section SHALL be headed by the representation it describes, and that
heading SHALL differ from every other heading the region draws. Section folds
are keyed by the heading's word, so two sections sharing one would share a
fold.

The section SHALL be drawn only where it has something to say. A heading with
no body SHALL NOT be drawn.

#### Scenario: The section names the representation
- **WHEN** a layer of any representation is active
- **THEN** the right region carries a section headed with that representation's name

#### Scenario: Folding one section does not fold another
- **WHEN** the user folds the geometry section on a grid layer
- **THEN** the grid's own section stays open

#### Scenario: The panel does not rearrange
- **WHEN** the user makes a layer of a different representation active
- **THEN** only the contextual section's contents change, and the sections above and below it keep their order

#### Scenario: Nothing to say draws nothing
- **WHEN** the engine has reported nothing about the active field layer
- **THEN** no field section is drawn, rather than a heading over an empty body

### Requirement: The inspector exposes only what the domain holds
The contextual section SHALL offer controls and readouts only for values this
application's domain or the engine can actually express for that layer. It
SHALL NOT present a control for a setting nothing reads, whatever a design
reference depicts.

Where a representation-specific control already exists elsewhere for a stated
reason — belonging to the stroke rather than the layer, or standing beside the
thing it acts on — it SHALL NOT be duplicated here.

#### Scenario: A depicted control with no domain behind it is absent
- **WHEN** a design reference shows a per-layer setting the domain cannot express
- **THEN** no control for it is drawn

#### Scenario: A field states its edit list
- **WHEN** a field layer is active and the engine has reported on it
- **THEN** the section states how many items the list holds and whether it has been collapsed

#### Scenario: A mesh states its topology contract
- **WHEN** a mesh layer is active
- **THEN** the section states that its brushes move existing vertices and neither add nor remove any

#### Scenario: A control that lives elsewhere is not repeated
- **WHEN** a representation's control already stands in the options bar or beside the layer stack
- **THEN** the contextual section does not draw a second copy of it

### Requirement: Naming a representation never converts a layer
Selecting, clicking or otherwise addressing a representation card SHALL NOT
change the active layer's representation. Crossing between representations
costs work, is not always reversible, and SHALL remain an explicit operation
confirmed where its cost is stated.

The application SHALL offer, beside the cards, exactly the crossings the domain
declares from the active representation — derived from the declared set rather
than listed, so a crossing that is added is offered and one that is removed
stops being. Invoking one SHALL aim the conversion panel at that crossing and
open it, and SHALL NOT perform the conversion.

#### Scenario: A card is inert
- **WHEN** the user clicks a representation the active layer is not
- **THEN** no conversion runs and the layer is unchanged

#### Scenario: A crossing opens the panel that states its cost
- **WHEN** the user invokes a crossing from the bar
- **THEN** the conversion is aimed at that crossing and the panel is shown, and the conversion has not run

#### Scenario: An already-open panel is not closed by aiming it
- **WHEN** the conversion panel is open and the user invokes a crossing
- **THEN** the panel stays open, aimed at the newly chosen crossing

#### Scenario: Only the crossings that exist are offered
- **WHEN** the active representation has no crossing to some other representation
- **THEN** no button offering it is drawn

### Requirement: The bar sheds its parts in a stated order
Where the window is too narrow for the bar to show everything, it SHALL give up
its explanatory phrases first, its heading second, and its crossings never. A
phrase given up SHALL remain available on hover.

A card SHALL always carry both an icon and a name. The bar SHALL scroll rather
than reduce a card to an icon alone.

#### Scenario: The phrases go first
- **WHEN** the region holding the bar is narrowed
- **THEN** the cards drop their phrases before anything else is lost, and the phrases appear on hover

#### Scenario: The crossings survive
- **WHEN** the bar cannot show everything
- **THEN** the crossings are still drawn

#### Scenario: A card never becomes an icon alone
- **WHEN** the bar has less room than even its shortest arrangement needs
- **THEN** it scrolls, and every card still shows an icon and a name

### Requirement: A layer can be crossed from its own row
The layer stack SHALL offer, from a layer's own menu, the crossings that layer
has — derived from the declared set for *that layer's* representation rather
than for the active one.

Invoking one SHALL make that layer active, aim the conversion at that crossing
with the in-place setting on, and open the conversion panel. It SHALL NOT
perform the conversion: a crossing costs work, a crossing into cells needs a
size chosen, and one that would exceed the budget is refused — all three are
stated in the panel.

In place means the source leaves as the result arrives and the result stands
where it stood, which is what a sculptor means by converting *this* layer.

#### Scenario: A layer offers its own crossings
- **WHEN** the user opens a mesh layer's menu
- **THEN** the crossings a mesh has are offered, and no others

#### Scenario: The crossing acts on the row it was asked of
- **WHEN** the user invokes a crossing from a layer that is not the active one
- **THEN** that layer is made active before the conversion is aimed

#### Scenario: A crossing from a row does not convert on the click
- **WHEN** the user invokes a crossing from a layer's menu
- **THEN** the conversion is aimed in place and the panel is shown, and the conversion has not run

### Requirement: A subdivision hierarchy is a subtool that can be worked on
The application SHALL hold a subdivision hierarchy on a layer, draw it from its
display level, place a pointer on it, and accept the brushes the tool table
offers there. Nothing about working on a hierarchy SHALL require the sculptor
to know that its cage is also a mesh layer in the document.

Detail stored on a hierarchy SHALL survive a change to the form beneath it: an
edit at a coarse level SHALL move the frames finer detail is stored in, so that
detail arrives at its new place at its own size rather than being smeared,
flattened or left pointing where the world is.

#### Scenario: Detail rides the form it stands on
- **WHEN** the user sculpts detail at a fine level and then edits the form at a
  coarse one
- **THEN** the detail is still there, the same size, on the same part of the
  surface, oriented to the form as it now sits, with the per-vertex fine-detail
  magnitudes unchanged across the coarse edit

#### Scenario: The pointer lands on the surface being drawn
- **WHEN** the user points at a hierarchy that has been sculpted
- **THEN** the brush is placed on the level the viewport is drawing, and not on
  the cage beneath it

#### Scenario: A dab is seen
- **WHEN** a dab lands on a hierarchy
- **THEN** the viewport draws the changed surface, including after the
  hierarchy's rebuildable caches have been released underneath it

### Requirement: A mesh becomes a cage only if it can be one
Crossing a mesh into a hierarchy SHALL refuse a mesh that cannot stand as a
subdivision cage, and SHALL name the fault rather than reporting a failure.
Nothing SHALL be repaired: mending a cage silently changes retopology the
sculptor paid for.

A refused crossing SHALL leave the source layer exactly as it was.

#### Scenario: A mesh with a degenerate face is refused by name
- **WHEN** the user crosses a mesh carrying a face with repeated or collinear
  corners into a hierarchy
- **THEN** the crossing is refused, and the sentence names that fault, so the
  sculptor goes back to the mesh rather than looking for a setting

#### Scenario: The source survives the refusal
- **WHEN** a crossing into a hierarchy is refused
- **THEN** the source layer is still a mesh layer and still carries its
  triangles

### Requirement: Adding a level is priced and refused rather than attempted
The application SHALL state what adding a level would cost before it is added,
and SHALL refuse a level that does not fit. The figure stated SHALL be the
**peak** during the build rather than what remains after it, because on a
constrained machine it is the high-water mark that ends the session.

A refused level SHALL leave the hierarchy exactly as deep as it was.

#### Scenario: The cost is beside the offer
- **WHEN** a hierarchy is the active layer
- **THEN** what one more level would occupy is shown beside the control that
  would add it, without adding one

#### Scenario: A level that does not fit is refused
- **WHEN** the user asks for a level whose peak exceeds the budget
- **THEN** the request is refused with the peak and the budget stated, and the
  hierarchy holds the levels it held before

#### Scenario: The refusal reaches the screen
- **WHEN** an operation on the active layer is refused
- **THEN** the reason is shown beside the viewport rather than only recorded

### Requirement: A hierarchy's two levels are controls
The application SHALL offer the sculpt level and the display level as separate
controls on the active hierarchy, and SHALL say when the two are apart —
because a dab landing on a coarse level while a fine one is drawn reads as a
brush that has stopped working.

#### Scenario: Both levels can be moved
- **WHEN** a hierarchy is the active layer
- **THEN** the level the brush writes on and the level being drawn are each
  offered, and moving one does not move the other

#### Scenario: Working apart from what is drawn is said
- **WHEN** the sculpt level and the display level differ
- **THEN** the interface says so

### Requirement: A hierarchy's sculpt level and display level are independent
The application SHALL model, for a layer holding a subdivision hierarchy, the
level a stroke writes on and the level the viewport draws as two separate
quantities. Moving one SHALL NOT move the other. Adding a level SHALL move both
to the level it added, which is what an artist means by subdividing.

#### Scenario: Moving the brush leaves the viewport where it was
- **WHEN** the sculpt level is changed on a hierarchy whose display level is
  finer
- **THEN** the display level is unchanged, and nothing is redrawn

#### Scenario: Moving the viewport leaves the brush where it was
- **WHEN** the display level is changed
- **THEN** the sculpt level is unchanged

#### Scenario: Subdividing moves both
- **WHEN** a level is added
- **THEN** both the sculpt level and the display level are the new level

#### Scenario: The interface can say the two disagree
- **WHEN** the sculpt level and the display level are different
- **THEN** the application can report that what is drawn is not what is being
  written

### Requirement: A hierarchy carries a stack of passes addressed by identity
The application SHALL model a hierarchy's sculpt layers as named, reorderable
passes carrying a strength, a visibility, a lock and whether they hold a stored
mask. Each pass SHALL be addressed by an identity that survives a reorder, and
SHALL NOT be addressed by its position in the stack.

#### Scenario: A reorder moves no vertex
- **WHEN** a pass is slid to another position in the stack
- **THEN** what the passes contribute to the surface is unchanged, and the
  application does not treat the reorder as an edit to the surface

#### Scenario: An identity outlives a reorder
- **WHEN** a pass is slid to another position
- **THEN** the same identity still names the same pass, with the same strength,
  visibility, lock and coverage

#### Scenario: A hidden pass contributes exactly nothing
- **WHEN** a pass is hidden
- **THEN** its contribution is zero rather than nearly zero

#### Scenario: A lock refuses a write and permits every property change
- **WHEN** a pass is locked
- **THEN** a stroke, a merge and a bake aimed at it are refused, and its name,
  strength, visibility and lock can still be changed

#### Scenario: A pass that is gone does not keep a stroke aimed at it
- **WHEN** the stack is read back without the pass that was active
- **THEN** the next stroke is routed into the form under the passes

### Requirement: Where a stroke on a hierarchy would land is chosen rather than inferred
The application SHALL let the destination of a stroke on a hierarchy be chosen
between the active pass, the form under the passes, and whichever of those two
applies, and SHALL be able to answer which of them a stroke would enter before
one is made. A destination naming the active pass where there is none SHALL be
answered as a refusal rather than as the form.

#### Scenario: With no destination chosen and no passes, a stroke enters the form
- **WHEN** a hierarchy carries no passes and no destination has been chosen
- **THEN** the application answers that a stroke would enter the form under the
  passes

#### Scenario: Choosing the form answers the form, whatever is active
- **WHEN** the destination is the form and a pass is active
- **THEN** the application answers that a stroke would enter the form

#### Scenario: Choosing a pass that is not there is a refusal, not a fallback
- **WHEN** the destination is the active pass and no pass is active
- **THEN** the application answers that the gesture would be refused, rather
  than answering with the form

### Requirement: A hierarchy carries passes that stay adjustable
The application SHALL let the sculptor make named passes on a subdivision
hierarchy, send a stroke into one, and afterwards dial its strength, hide it,
lock it, reorder it, fold it into the pass below, bake it into the form, or
remove it.

A pass's strength SHALL remain adjustable for as long as the pass exists,
independently of the gesture that filled it. Dialling a pass SHALL replay no
stroke: a pass at zero strength SHALL contribute exactly nothing, and returning
it to full SHALL restore exactly what was there.

Acting on a pass SHALL NOT enter the edit history. A pass is a property of the
stack rather than a step in the work.

#### Scenario: A pass is dialled long after the stroke that filled it
- **WHEN** the user strokes into a pass, releases the pointer, and later moves
  that pass's strength to zero
- **THEN** the surface returns to what it was before the stroke, and moving the
  strength back restores the stroke exactly

#### Scenario: Hiding a pass removes its contribution
- **WHEN** the user hides a pass
- **THEN** the surface is exactly what it would be with that pass at zero
  strength

#### Scenario: Undo does not take back a slider
- **WHEN** the user dials a pass and then undoes
- **THEN** the last edit is undone, and the pass keeps the strength it was
  given

### Requirement: Where a stroke lands is a row the sculptor selects
The interface SHALL show the stack of passes under the layer they stand on, and
SHALL show the form beneath them as a row of its own. Exactly one of those rows
SHALL be selected at a time, and the next stroke SHALL enter whichever it is.

Selecting the form SHALL leave every pass untouched, so that a sculptor can
correct the surface under a set of passes without disturbing them.

A hierarchy with no passes SHALL still show the form's row, so that the
sculptor can see where a stroke is going before there is anywhere else for it
to go.

#### Scenario: A new pass takes the next stroke
- **WHEN** the user adds a pass
- **THEN** it is selected, and the next stroke goes into it

#### Scenario: The form is selected and the passes are left alone
- **WHEN** the user selects the form's row and strokes
- **THEN** the surface under the passes changes and every pass keeps exactly
  what it held

### Requirement: Reordering a pass is organisation and never geometry
The interface SHALL let the sculptor reorder passes by dragging a row, and that
reorder SHALL move no vertex: the passes compose as a sum, so their order
decides where a row is drawn and never what the surface is.

#### Scenario: A drag reorders the list and nothing else
- **WHEN** the user drags one pass onto another
- **THEN** the two swap places in the list and the surface is unchanged

### Requirement: The composition is held while a stroke is open
While a gesture is in progress the application SHALL refuse the changes that
would recompose the surface — a strength, a visibility, a reorder, an addition,
a removal — and SHALL say that it is waiting for the pointer rather than
failing silently. A rename, a lock and a change of which row is selected move
no vertex and SHALL be accepted.

#### Scenario: A slider moved mid-gesture is refused with a reason
- **WHEN** the user moves a pass's strength while a stroke is still open
- **THEN** the change is refused, the interface says the brush has to be
  released, and the change works as soon as it is

### Requirement: What the stack costs is shown, and nothing is enforced against it
The interface SHALL show what the stack of passes occupies and how much surface
it covers, and SHALL offer to release the storage a stroke that undid itself
left behind. No limit SHALL be enforced against that figure: a cap that
silently stopped recording would leave a pass on the surface and un-dialable.

#### Scenario: A large stack is called out rather than capped
- **WHEN** the stack passes the size at which it is worth releasing
- **THEN** the figure is drawn in the interface's warning colour and the offer
  to release stands, and every pass goes on taking strokes
