## ADDED Requirements

### Requirement: The viewport renders geometry produced by the engine's meshers
The viewport SHALL display triangles produced by ClayCore's meshers. It SHALL use the surface-nets preview mesher for interactive display and marching tetrahedra where a watertight, 2-manifold result is required. The application SHALL NOT reimplement any distance function, combine operator, blend profile or deformer in a shading language.

#### Scenario: No field math in shaders
- **WHEN** the WGSL shader sources are inspected
- **THEN** they contain shading, transform and display code only, and no signed-distance primitive, blend or deformer evaluation

#### Scenario: The displayed surface is the document's surface
- **WHEN** a document is displayed in the viewport and separately meshed through the engine at the same resolution
- **THEN** the two surfaces are the same geometry, because the viewport did not compute one of its own

### Requirement: Edits re-mesh only the region they touched
After an edit, the application SHALL re-mesh only the bricks the edit's influence bound marked dirty, by passing the engine's dirty key set as the meshing subset, and SHALL upload only the affected geometry to the renderer. It SHALL NOT re-mesh or re-upload the whole model for a local edit.

#### Scenario: A brush dab costs what it touched
- **WHEN** a single brush dab is applied to a large model
- **THEN** the keys meshed are those the dab's influence bound intersects, and every other brick's geometry is left untouched in both the cache and the renderer's buffers

#### Scenario: Uploads are patched by key range
- **WHEN** a subset re-mesh returns per-key vertex and index ranges
- **THEN** the renderer overwrites exactly those sub-ranges of its buffers rather than rebuilding them

#### Scenario: A stale mesh result is discarded
- **WHEN** a brick is re-dirtied while a meshing request for it is in flight and the older result arrives afterwards
- **THEN** the older result is discarded and the newer dirty state is preserved

### Requirement: Mesh buffers are compacted off the interaction path
Because meshed vertices are welded across brick seams, a key's range may be overwritten but SHALL NOT be freed in isolation. The application SHALL therefore reclaim fragmented buffer space by re-meshing the whole surface as a background operation, scheduled when it will not interrupt sculpting, and SHALL NOT perform compaction as part of an edit.

#### Scenario: Compaction never blocks a stroke
- **WHEN** buffer fragmentation reaches the configured threshold while the user is sculpting
- **THEN** compaction is deferred until the stroke ends and runs without interrupting input

#### Scenario: Overwriting a range does not corrupt neighbours
- **WHEN** a key's vertex range is overwritten by a re-mesh while a neighbouring key's triangles reference vertices welded across the seam
- **THEN** the rendered surface remains correct across that seam

### Requirement: Vertex data is written to the GPU in one pass
The application SHALL obtain mesh vertices in its own interleaved layout through the engine's layout-directed copy, writing directly into mapped GPU memory. It SHALL NOT read the engine's attribute arrays and interleave them itself.

#### Scenario: One pass into mapped memory
- **WHEN** a re-mesh result is uploaded
- **THEN** the vertices are written once, in the renderer's layout, into the mapped buffer

#### Scenario: A layout naming an absent attribute is rejected
- **WHEN** the requested layout names an attribute the mesh does not carry
- **THEN** the copy is refused with a stated reason rather than writing a buffer that is wrong without looking wrong

### Requirement: Surfaces are shaded with a selectable MatCap material
The viewport SHALL shade the sculpt with a MatCap material chosen from a built-in set, presented as sphere previews. The material SHALL affect display only and SHALL NOT be written into the document's geometry or exported mesh data.

#### Scenario: Switching material does not modify the document
- **WHEN** the user selects a different MatCap
- **THEN** the display changes, the document is not marked modified, and the undo history gains no entry

#### Scenario: Vertex colors are honored where present
- **WHEN** the displayed mesh carries vertex colors from a palette-indexed voxel layer
- **THEN** those colors modulate the MatCap shading rather than being discarded

### Requirement: The viewport offers orbit, pan, zoom and framing
The camera SHALL support orbit, pan and zoom from pointer and trackpad input, SHALL frame the whole document on demand, and SHALL frame the current selection on demand.

#### Scenario: Framing an empty document
- **WHEN** the user requests frame-all on a document with no visible geometry
- **THEN** the camera moves to a defined default view rather than to an undefined or degenerate position

### Requirement: Standard view presets are directly reachable
The viewport SHALL offer Perspective, Front, Side and Top presets, reachable in one action, showing which is active. Selecting an orthogonal preset SHALL switch to an orthographic projection; Perspective SHALL restore the perspective projection.

#### Scenario: Preset selection preserves framing
- **WHEN** the user switches from Perspective to Front
- **THEN** the camera looks along the front axis with the subject still framed, rather than resetting the zoom to a default distance

### Requirement: A navigation gizmo shows and sets orientation
The viewport SHALL display an axis gizmo indicating the current orientation with labelled X, Y and Z axes. Activating an axis on the gizmo SHALL orient the camera along that axis.

#### Scenario: Gizmo reflects the camera
- **WHEN** the camera is orbited
- **THEN** the gizmo's axes update to match the new orientation on the same frame

### Requirement: The brush cursor previews the brush in the scene
While a sculpting tool is active and the pointer is over the surface, the viewport SHALL draw a cursor showing the brush's projected radius on the surface at the pointer's position, and SHALL indicate the surface point at its centre. The cursor SHALL follow the brush size setting.

#### Scenario: Cursor tracks size changes live
- **WHEN** the user changes the brush size while the pointer is over the surface
- **THEN** the cursor radius updates immediately, without requiring a stroke

#### Scenario: Cursor off the surface
- **WHEN** the pointer is over the viewport but not over any surface
- **THEN** the cursor indicates that no surface is under the pointer rather than drawing a radius at an arbitrary depth

### Requirement: Reference overlays are available and unobtrusive
The viewport SHALL offer a ground grid and a symmetry-plane indicator as toggleable overlays. Overlays SHALL render behind or beneath the sculpt in visual weight, SHALL never obscure the silhouette, and SHALL be excluded from every export.

#### Scenario: The symmetry plane does not cross the form
- **WHEN** a symmetry plane is shown with the camera inside the plane's extent
- **THEN** the indicator is the plane's outline and centre lines only, drawn at
  a fraction of the accent, with no lattice of lines across the sculpt

#### Scenario: Overlays never reach an export
- **WHEN** a mesh is exported with the grid and symmetry plane visible
- **THEN** the exported file contains only the sculpted geometry

### Requirement: Level of detail follows the viewing distance
The viewport SHALL use the brick cache's LOD mip levels for regions far from the camera and full-resolution bricks for regions near it, and SHALL derive mips from up-to-date full-resolution data rather than from stale data.

#### Scenario: Detail is restored on approach
- **WHEN** the camera moves close to a region previously displayed at a reduced LOD
- **THEN** that region is displayed at full resolution without requiring an edit or a manual refresh

### Requirement: A surface the device cannot hold is drawn coarser, not fatally
The renderer SHALL request the adapter's own buffer ceiling, SHALL report a
graphics validation error rather than terminate on one, and where a surface at
the level being drawn would still exceed the device's largest buffer SHALL
refuse the layout, keep what is on screen, and drop to the coarse level of
detail until the surface fits again. An engine mesh carrying no vertices SHALL
be read as empty rather than reported as a failed copy.

#### Scenario: A subtool is scaled past what the device can draw at full detail
- **WHEN** a whole subtool is scaled up until its full-resolution surface is
  larger than the device's largest buffer
- **THEN** the application keeps running, the surface is drawn at the coarse
  level, and the geometry panel says the detail is reduced

#### Scenario: A reservation past the ceiling is refused
- **WHEN** the renderer is asked to reserve more vertices than the device's
  ceiling holds
- **THEN** the reservation is refused and the existing buffers are unchanged

### Requirement: Rendering device loss is recovered, not fatal
If the WebGPU device is lost, the application SHALL recreate it and its resources and resume rendering, SHALL NOT lose the open document, and SHALL inform the user that rendering was reset.

#### Scenario: Device loss preserves the document
- **WHEN** the rendering device is lost during a session with unsaved work
- **THEN** rendering resumes on a recreated device and no document state or undo history is lost
