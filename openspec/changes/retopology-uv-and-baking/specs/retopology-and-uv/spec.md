## ADDED Requirements

### Requirement: A mesh subtool can be retopologised to quads
A mesh subtool SHALL be retopologisable through `cyber_remesh`, with the quad
methods the pinned release carries and a target quad count.

The result SHALL replace the subtool's topology **in place**, in one undo entry,
as *Refazer a malha* already does. The stack is not the record of an operation
the history records.

The commit SHALL be a compare-and-swap against the layer revision read before
the work was dispatched, and SHALL refuse rather than overwrite when the layer
has moved since — the work runs off the interface thread and the source remains
strokeable while it does.

The operation SHALL be offered on mesh subtools only. A field is not a mesh, and
crossing one for the sculptor silently would perform a representation change
with its own cost and its own undo entry.

#### Scenario: A sculpted mesh is retopologised
- **WHEN** the operation completes
- **THEN** the quads stand in a new subtool and the source is untouched

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
