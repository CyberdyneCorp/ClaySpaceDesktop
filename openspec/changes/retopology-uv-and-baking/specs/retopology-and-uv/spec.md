## ADDED Requirements

### Requirement: A mesh subtool can be retopologised to quads
A mesh subtool SHALL be retopologisable through `cyber_remesh`, with the quad
methods the pinned release carries and a target quad count.

The accepted result SHALL become a new mesh subtool by default, standing where
the source stands, leaving the sculpt source intact and available for
comparison. Its creation SHALL be one undo entry, and undoing it SHALL remove
the new subtool whole. A sculptor MAY ask for the source subtool to be rebuilt
in place instead; that SHALL also be one undo entry.

Only the active subtool SHALL be read as the source. Other visible subtools
SHALL NOT be folded into it.

The result SHALL be published only if the source still stands at the layer
revision read before the work was dispatched, whichever placement was asked
for. A source that has moved, or has gone, SHALL receive nothing, and the
discard SHALL be reported — the work runs off the interface thread and the
source remains strokeable while it does. In place, the commit SHALL also be a
compare-and-swap in the engine that leaves the layer unchanged when it refuses.

The operation SHALL be offered on mesh subtools only. A field is not a mesh, and
crossing one for the sculptor silently would perform a representation change
with its own cost and its own undo entry.

Retopology SHALL be a named workflow, available from the interface and agent
catalogue. It SHALL run as a cancellable job with progress and a preview of
the result. The sculptor SHALL explicitly accept or discard that preview.

#### Scenario: A sculpted mesh is retopologised
- **WHEN** the operation completes and its preview is accepted
- **THEN** the quads stand in a new subtool and the source is untouched

#### Scenario: A result is discarded
- **WHEN** the sculptor discards a completed retopology preview
- **THEN** neither a new subtool nor a history entry is created

#### Scenario: The source moves while the job runs
- **WHEN** the source subtool is edited, or removed, after the job started and
  before its result is published
- **THEN** the result is discarded with a notice, and neither a new subtool nor
  a history entry is created

#### Scenario: A running job is cancelled
- **WHEN** the sculptor or an agent cancels a retopology in progress
- **THEN** nothing is published and the document is unchanged

#### Scenario: The source is rebuilt in place
- **WHEN** the sculptor asks for the retopology in place
- **THEN** the source subtool holds the quads, the subtool count is unchanged,
  and one undo puts the original topology back

### Requirement: A retopology is one undo entry
Placing the result SHALL be one entry in the history, as a crossing and a
boolean already are.

#### Scenario: A retopology is taken back
- **WHEN** undo is invoked once
- **THEN** the new subtool is gone and the scene is as it was

### Requirement: A mesh that has moved on can be conformed rather than redone
`cyber_conform` SHALL be offered where a retopologised subtool's source has been
sculpted since: it re-snaps the low-poly onto the new surface **preserving its
topology exactly**.

It SHALL report the maximum and RMS deviation and the count of vertices past a
threshold, and those SHALL reach the sculptor. The engine completes and flags
rather than silently stretching, and a host that drops the flags turns that into
a silent stretch again.

#### Scenario: The sculpt moved further than the conform could follow
- **WHEN** the conform completes
- **THEN** the deviation is reported, and the vertices past the threshold are
  named as a count rather than hidden behind a success

### Requirement: A UV layout can be produced automatically or along chosen seams
`cyber_uv_atlas` SHALL provide the one-call path, reporting chart count,
conformal distortion, flip count and packing efficiency.

`cyber_uv_unwrap_seams` SHALL provide the counterpart that cuts **exactly** the
seams given. An empty seam set SHALL mean *do not cut* and SHALL NOT mean
*decide for me* — the two are different requests and the engine distinguishes
them.

#### Scenario: An empty seam set is unwrapped along
- **WHEN** the mesh carries no marked seams
- **THEN** it is parameterised without cuts rather than auto-seamed

### Requirement: Guides and density are retopology inputs
The retopology workflow SHALL accept surface flow guides and a painted density
field as retopology data, independent of sculpt brushes. Guides and density
SHALL be visible and editable before a job starts and SHALL be passed to the
remesher only when that job is requested. Their edits SHALL be undoable.

Guides SHALL carry ordered surface points, strength, influence radius and an
orientation or topology mode. Density SHALL be authored as surface samples and
translated to per-vertex remesher guidance at the adapter. Both SHALL survive
document save and reopen. A retopology gesture SHALL NOT be dispatched as a
sculpt stroke, and authoring either input SHALL leave the sculpt geometry
unchanged. The same controls SHALL work on Mesh, Dynamic, Multires, Voxel and
SDF sources through a temporary mesh of the active subtool.

#### Scenario: Density changes the requested topology
- **WHEN** the same mesh is retopologised with and without a higher-density region
- **THEN** the accepted mesh has measurably more quads in that region, while the source sculpt is unchanged

#### Scenario: A guide steers flow
- **WHEN** a flow guide is drawn across a source mesh and the job runs
- **THEN** the job receives that guide as retopology input rather than modifying the source surface

#### Scenario: Guide and density edits survive a round trip
- **WHEN** an artist saves and reopens a document with edited guides and density
- **THEN** the same guides, control points, modes, strengths and density samples are available for the next retopology run

#### Scenario: Retopology mode captures the pointer
- **WHEN** a pointer gesture begins while guide or density authoring is active
- **THEN** the retopology guidance changes and the sculpt geometry and sculpt history do not

### Requirement: Optional UVs survive acceptance
The retopology workflow SHALL offer optional automatic UV generation for its
result. The preview SHALL report UV metrics and the accepted mesh SHALL retain
the resulting UV coordinates. Declining UV generation SHALL leave the result
without newly generated UVs. A failed UV step SHALL be reported and SHALL NOT
be silently presented as a UV-carrying success.

UV generation SHALL be off by default, in the panel and in the agent's
`retopo set` (`uvs`). The report SHALL carry the engine's chart count, angle
distortion, coverage and seam count, and SHALL name flipped and projected
(fallback) charts as defects rather than as figures.

The engine writes a layout per face corner; a mesh layer stores one UV per
vertex. The accepted mesh SHALL therefore duplicate exactly the vertices on a
seam, one copy per distinct UV, and SHALL carry the welded mesh's normals on
every copy so a seam is not shaded into the surface. The authored quad edges
SHALL still be drawn.

A failed layout SHALL NOT fail the retopology: the quads SHALL be placed without
UVs, and the reason SHALL reach the sculptor and the agent. A cancellation
during the layout SHALL cancel the whole run.

#### Scenario: A result with UVs is accepted
- **WHEN** automatic UV generation is requested and the retopology preview is accepted
- **THEN** the new mesh subtool retains the previewed UV coordinates after save and reload

#### Scenario: UVs survive the history
- **WHEN** an accepted result carrying UVs is undone and redone
- **THEN** the layer that returns carries the same UV coordinates

#### Scenario: UVs are declined
- **WHEN** a retopology result is accepted without requesting UV generation
- **THEN** no automatic atlas is generated for that result

#### Scenario: The layout is refused
- **WHEN** UV generation is requested and the engine refuses the layout
- **THEN** the quads are placed without UVs, the outcome reports `failed` with
  the engine's reason, and the notice says the mesh carries no UVs

### Requirement: A layout on a layer can be seen on it
A mesh layer carrying UVs SHALL offer a UV display with three choices:
the layer's own material (the default), a checker in UV space over the
material, and the same checker tinted a distinct colour per island. The
display SHALL draw the layout's seams over either checker in a colour of their
own. It SHALL be offered in the active mesh layer's inspector only when that
layer carries UVs, and to the agent as `view set_uv_display`
(`off` / `checker` / `islands`); `state.presentation.uv_display` SHALL report
what the viewport shows.

The display SHALL be presentation only: it SHALL NOT change a vertex, a UV,
the document, its modified mark or its history. It SHALL draw the active layer
only, on the triangles and at the placement the viewport already draws for
that layer, and SHALL fall back to the layer's own material while the surface
is drawn through (a cage, or a dialled-back opacity). A choice made while the
active layer carries no layout SHALL be kept and shown on the next active
layer that does.

An island SHALL be a set of triangles joined through shared vertices, and a
seam SHALL be a border edge whose two positions are also those of another
border edge — so a mesh's own open border is not drawn as a seam. For an
accepted retopology the display SHALL find one island per chart and one seam
per seam edge the engine reported.

#### Scenario: The checker is drawn on the accepted result
- **WHEN** a retopology result carrying UVs is active and the checker is chosen
- **THEN** the viewport draws that layer with alternating squares in UV space
  and its seams drawn over them, and choosing the material again draws the
  layer exactly as before

#### Scenario: Islands are told apart
- **WHEN** the island display is chosen on a layer whose layout has several charts
- **THEN** each chart is drawn in a colour of its own, and the number of
  islands equals the engine's chart count

#### Scenario: No layout, no display
- **WHEN** the active layer carries no UVs
- **THEN** the inspector offers no UV display, the viewport draws the layer's
  own material whatever was chosen, and `state.presentation.uv_display` is
  `off`

#### Scenario: The display is not an edit
- **WHEN** the UV display is changed
- **THEN** no history entry is created and the document is not marked modified

### Requirement: A finished retopology is held as a preview until it is answered
A retopology job that lands on a source still at the revision it read SHALL be
held as a preview rather than placed. While it is held, the document SHALL be
unchanged: no subtool, no history entry, no modified mark, and nothing written
by a save.

The sculptor SHALL answer it from the retopology panel (Accept, Discard) and an
agent through `retopo accept` and `retopo discard`; `outcomes.retopology.pending`
SHALL report whether a result is held. Accepting SHALL place exactly what the
job placed on landing before this requirement: the same subtool, standing in
the same place, carrying the same UVs, in one history entry, beside the source
or over it as the run was asked. Discarding SHALL leave the document and its
history exactly as they were, and the discarded result SHALL NOT be placeable
afterwards. Either answer with nothing held SHALL be refused with a reason.

A held preview SHALL be dropped, with a notice, when its source moves — a
stroke, an undo, a redo — and an accept that arrives before that is noticed
SHALL be refused by the same check. Starting another run SHALL drop it. Saving
SHALL keep it held and acceptable. Replacing the document — new, open, revert —
SHALL drop it without asking, since it holds no work of the document's.

The preview SHALL be drawn standing where acceptance would draw it, lit through
the surface's own pipeline, with its authored edges under the polyframe. Where
the source is a carried subtool the preview SHALL be drawn in place of it —
its triangles and its polyframe lines left out while the preview is held. A
field source SHALL stay drawn under the preview. The UV display SHALL describe
the preview while one is held: offered and shown where the preview carries a
layout, whatever the active layer carries.

#### Scenario: A held preview is accepted
- **WHEN** a retopology lands and the sculptor or an agent accepts it
- **THEN** the subtool placed, its UVs and the history entry are those the job
  placed on landing, and the preview drawn before acceptance is the accepted
  layer as the viewport then draws it

#### Scenario: A held preview is discarded
- **WHEN** a held preview is discarded
- **THEN** the saved document is byte-equal to the one saved before the run,
  the history depth and the next undo are unchanged, the source is drawn again
  exactly as before, and accepting afterwards is refused

#### Scenario: The source moves while a preview is held
- **WHEN** the source is stroked, or an undo or redo moves it, while its
  preview is held
- **THEN** the preview is dropped with a notice and nothing can be accepted

#### Scenario: A new run while a preview is held
- **WHEN** a retopology is started while a preview is held
- **THEN** the held preview is dropped before the source is read, and the new
  result is the one held when it lands

#### Scenario: A save while a preview is held
- **WHEN** the document is saved while a preview is held
- **THEN** the file carries no trace of the preview and the preview can still
  be accepted

#### Scenario: The document is replaced while a preview is held
- **WHEN** a new document is made or another is opened while a preview is held
- **THEN** the preview is dropped without a prompt or a notice

#### Scenario: The checker is drawn on a held preview
- **WHEN** a held preview carries UVs and the checker or island display is chosen
- **THEN** the preview is drawn as its layout with its seams, and
  `state.presentation.uv_display` reports that display whatever the active
  layer carries

### Requirement: A layout is drawn in its UV square
Wherever a UV display is chosen and the active layer, or a held retopology
preview, carries UVs, the interface SHALL draw the layout in the unit UV square:
every triangle at its UVs — each island in the tint the island display gives
it on the surface, or one neutral fill under the plain checker — with both
sides of every seam in the seam colour. It SHALL disappear when the material
display is chosen.

The square SHALL be presentation only: it SHALL NOT push a command, enter the
history or mark the document modified. `state.presentation.uv_layout` SHALL
report its islands, seams and triangles while it is drawn, and be absent
otherwise.

#### Scenario: A retopology's layout is drawn flat
- **WHEN** a retopology with UVs is held or accepted and a UV display is chosen
- **THEN** the square draws one island per chart the engine reported and one
  seam per seam edge, and `state.presentation.uv_layout` says so

#### Scenario: The material hides the square
- **WHEN** the material display is chosen
- **THEN** the square is not drawn and `state.presentation.uv_layout` is absent

### Requirement: Maps are baked from the field where a field is what exists
Normal, ambient occlusion, curvature and cavity SHALL be baked through
`cyber_bake_field`, sampling ClayCore's field directly, so that the cage ray is
sphere-traced through the actual surface rather than a tessellation of it and
normals come from exact gradients.

The conversions in the retopology-bridge capability SHALL apply. Every other map
still requires a target mesh and SHALL take the raycast path, whose output the
engine documents as bit-identical.

#### Scenario: A normal map is baked from a sculpt with no exported high-poly
- **WHEN** the bake runs with a field evaluator attached
- **THEN** it completes with no target mesh, sampling the field
