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
