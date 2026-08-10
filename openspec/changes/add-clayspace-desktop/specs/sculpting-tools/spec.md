## ADDED Requirements

### Requirement: Every tool maps to a documented engine verb
Each sculpting tool the interface presents SHALL correspond to a documented ClayCore verb reached through the C ABI. The application SHALL NOT present a tool that has no engine counterpart, and SHALL NOT bind a label to a verb whose behavior differs from what the label states.

The bound vocabulary SHALL be: Padrão (relief along a stroke), Inflar (relief / voxel inflate), Suavizar and Relaxar (field relax / voxel smooth), Mover (move brush), Puxar (snakehook), Pinçar (magnify negative / voxel pinch), Magnify (magnify positive), Raspar (voxel scrape), Planar and Polir (flatten in cut-only mode), Preencher (fill cavities), Nudge (voxel smudge), Camada (clamped-accumulation stroke preset), Máscara (mask stroke), and Trim (cut tool).

#### Scenario: A tool's label matches its verb
- **WHEN** the user selects Planar and applies it to a surface
- **THEN** the engine's flatten operation runs in cut-only mode, planing without filling, as the label states

#### Scenario: No orphan tools
- **WHEN** the tool registry is enumerated
- **THEN** every entry names the engine entry point it invokes, and none is unbound

### Requirement: A tool unavailable on the active layer is disabled with a reason
Where a verb exists on one representation only — carve-with-alpha is voxel-side, flatten requires a region on the SDF side — the tool SHALL be presented as disabled on layers that cannot accept it, together with the reason. It SHALL NOT be presented as enabled and then do nothing.

#### Scenario: A voxel-only verb on an SDF layer
- **WHEN** an SDF layer is active and a voxel-only tool is shown
- **THEN** the tool is disabled and states that it applies to voxel layers

#### Scenario: Selecting a layer that supports the active tool re-enables it
- **WHEN** the user activates a voxel layer while a voxel-only tool is selected
- **THEN** the tool becomes enabled without the user reselecting it

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

#### Scenario: Mirrored edits are symmetric
- **WHEN** X symmetry is active and the user sculpts on one side
- **THEN** the mirrored edit is applied on the other side within the same edit, and undoing the edit removes both

#### Scenario: Symmetry off leaves prior work untouched
- **WHEN** the user disables symmetry
- **THEN** existing geometry is unchanged and only subsequent edits are asymmetric

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
