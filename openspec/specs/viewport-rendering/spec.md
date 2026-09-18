# viewport-rendering Specification

## Purpose
What the viewport draws and how: geometry the engine's meshers produced,
shaded with a MatCap, navigated and framed, with occlusion, level of detail and
the overlays and scaffolding that stand over the form — and never a surface the
renderer computed for itself.
## Requirements
### Requirement: The viewport renders geometry produced by the engine's meshers
The viewport SHALL display triangles produced by ClayCore's meshers, reached
through the mesh readback path, and SHALL NOT reconstruct a surface of its own.
It SHALL use the surface-nets preview mesher for interactive display and
marching tetrahedra where a watertight, 2-manifold result is required. The
application SHALL NOT reimplement any distance function, combine operator, blend
profile or deformer in a shading language.

This holds for every shader this renderer gains — occlusion, depth reduction,
bilateral upsample, cavity, tone mapping, studio lighting and shadowing are
display code over a depth buffer and a mesh, and none of them evaluates the
field that produced either.

Where the geometry carries per-vertex colour — a voxel layer's palette, a mesh
layer's colour attribute — the viewport SHALL modulate the material with it.
The field surface is meshed without colour and its vertices carry the identity
value, so enabling the modulation SHALL leave a field surface unchanged.

#### Scenario: No field math in shaders
- **WHEN** the WGSL shader sources are inspected
- **THEN** they contain shading, transform and display code only, and no signed-distance primitive, blend or deformer evaluation

#### Scenario: The displayed surface is the document's surface
- **WHEN** a document is displayed in the viewport and separately meshed through the engine at the same resolution
- **THEN** the two surfaces are the same geometry, because the viewport did not compute one of its own

#### Scenario: Painted colour is visible
- **WHEN** a voxel layer is painted and the viewport is drawn
- **THEN** the painted region is drawn in that colour

#### Scenario: A field surface is not tinted by the switch
- **WHEN** the same SDF scene is captured before and after colour modulation is
  enabled
- **THEN** the two images agree to within the rasteriser's own precision, which
  is not the same as being identical: interpolating a colour that is one at
  every vertex is a ratio of two sums, so a silhouette pixel may still round
  the other way

### Requirement: Edits re-mesh only the region they touched
After an edit, the application SHALL re-mesh only the bricks the edit's influence bound marked dirty, by passing the engine's dirty key set as the meshing subset, and SHALL upload only the affected geometry to the renderer. It SHALL NOT re-mesh or re-upload the whole model for a local edit.

An edit that moves vertices without changing topology SHALL upload vertices
alone and SHALL NOT re-upload the index buffer. GPU buffers SHALL grow
geometrically rather than to the exact size required, and SHALL NOT shrink
during interaction.

#### Scenario: A brush dab costs what it touched
- **WHEN** a single brush dab is applied to a large model
- **THEN** the keys meshed are those the dab's influence bound intersects, and every other brick's geometry is left untouched in both the cache and the renderer's buffers

#### Scenario: Uploads are patched by key range
- **WHEN** a subset re-mesh returns per-key vertex and index ranges
- **THEN** the renderer overwrites exactly those sub-ranges of its buffers rather than rebuilding them

#### Scenario: A deformation uploads no indices
- **WHEN** a brush moves the vertices of a carried mesh without changing its
  topology
- **THEN** only the affected vertex ranges are written, and the index buffer is
  not rewritten

#### Scenario: Growth does not reallocate per dab
- **WHEN** geometry grows gradually across many edits
- **THEN** the buffer is reallocated a number of times logarithmic in the final
  size rather than once per growth

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

The ground grid SHALL dissolve before it reaches its own extent, so that it
draws no boundary around the scene. The dissolve SHALL vary along each line
rather than per line, so that a line running past the form thins as it leaves
it.

The grid SHALL distinguish major lines from minor ones, so that a distance can
be counted rather than estimated.

#### Scenario: The grid draws no rectangle around the scene
- **WHEN** the ground grid is shown
- **THEN** its outermost lines have reached the viewport's ground tone, and no edge is visible where the grid ends

#### Scenario: The grid is strongest under the form
- **WHEN** the ground grid is shown
- **THEN** the lines near the origin are stronger than the same lines further out

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

### Requirement: Occlusion is computed below the display resolution and upsampled with regard for edges
The occlusion pass SHALL run at a fraction of the scene resolution — half by
default — over a reduced, single-sampled depth target, and its result SHALL be
brought back to full resolution by a filter weighted by both screen distance
and depth similarity rather than by an unweighted box average.

The reduction SHALL take the closest covered depth of the samples it stands
for, not their average: an average of a foreground and a background that met
at a silhouette describes a surface that is not there.

#### Scenario: Occlusion does not cross a silhouette
- **WHEN** a near form is drawn against a far one with the camera positioned so
  their silhouettes meet
- **THEN** the far surface is not darkened by the near one's occlusion beyond
  the reach the radius allows, and no halo is drawn around the near silhouette

#### Scenario: Contact shadows survive the lower resolution
- **WHEN** two surfaces meet in a deep crease
- **THEN** the crease darkens by at least as much as it did at full resolution,
  rather than being washed out by the upsample

#### Scenario: The occlusion target is smaller than the scene
- **WHEN** the framebuffer is created for a scene of a given size
- **THEN** the occlusion target's extent is the configured fraction of it, and
  the occlusion pass is told that extent separately from the scene viewport

### Requirement: Occlusion does not depend on the device multisampling
The occlusion passes SHALL read a reduced single-sampled depth target rather
than the scene's multisampled depth buffer, and SHALL therefore run at every
sample count the device offers, one included.

#### Scenario: A device that will not multisample still shades occlusion
- **WHEN** the scene is rendered on a device or format that supports only one
  sample per pixel
- **THEN** the surface is still darkened where it closes in on itself, rather
  than the occlusion passes being skipped

### Requirement: Occlusion is scaled to the size of what is being sculpted
The occlusion radius and bias SHALL be derived from the radius of the geometry
being displayed rather than fixed in absolute units, so that a model and the
same model scaled by any factor are shaded alike.

#### Scenario: The same form at two scales shades the same
- **WHEN** a form is displayed, and then displayed again scaled by a hundred
  with the camera framed to match
- **THEN** the two frames darken their folds by comparable amounts, rather than
  one showing no occlusion and the other showing total occlusion

### Requirement: Interactive frames render at a lower quality than idle ones
The viewport SHALL carry an explicit quality state, and the application — not
the renderer — SHALL choose it from what the pointer is doing. A stroke in
progress SHALL drop occlusion sample count, disable the cavity term and
disable temporal accumulation, whatever quality profile is selected.

The state SHALL NOT change on every pointer event: it SHALL fall to the
interactive tier immediately on pointer down and rise again only after the
pointer has been still for a stated interval.

The quality profile SHALL be selectable by the user. Choosing one SHALL change
what an idle frame is drawn with and SHALL NOT change what is drawn, so it
SHALL emit no command and SHALL enter no history.

#### Scenario: A stroke does not pay for idle quality
- **WHEN** a stroke is in progress under the Presentation profile
- **THEN** the frames drawn during the stroke use the interactive occlusion
  sample count and draw no cavity term

#### Scenario: Quality does not oscillate
- **WHEN** the pointer is pressed and released repeatedly in quick succession
- **THEN** the quality state does not rise between the events, and rises only
  once the pointer has been idle for the stated interval

#### Scenario: The renderer is told, not asked
- **WHEN** the renderer draws a frame
- **THEN** it takes the quality and interaction state it was given, and reads
  no pointer or input state of its own

#### Scenario: A profile can be chosen
- **WHEN** the user chooses a viewport quality profile
- **THEN** the governor is given it, and subsequent idle frames are drawn at that profile's ceiling

#### Scenario: Choosing a profile changes no document
- **WHEN** the user chooses a viewport quality profile
- **THEN** no command is emitted and the edit history is unchanged

### Requirement: Temporal occlusion accumulation never trails a brush
Where temporal accumulation of occlusion is enabled, the history SHALL be
rejected on a camera cut, a viewport resize, a projection change, a depth
mismatch beyond a stated tolerance, and any edit to the geometry.

#### Scenario: An edit does not leave a shadow behind
- **WHEN** geometry is changed under the brush with temporal accumulation on
- **THEN** the occlusion in the changed region reflects the new geometry on the
  next frame, with no residue of the old

### Requirement: Depth range follows the scene rather than being fixed
The camera's near and far planes SHALL be derived from the viewing distance and
the bounds of what is displayed, smoothed so that the range does not change
abruptly between frames, and the viewport SHALL use a reversed-Z depth mapping
— near at one, far at zero — so that floating-point depth precision is
distributed usefully across the range.

#### Scenario: A close zoom on a small form does not clip
- **WHEN** the camera is zoomed close to a form far smaller than the previous
  fixed near plane allowed for
- **THEN** the surface is drawn without near-plane clipping

#### Scenario: Thin overlapping shells do not fight
- **WHEN** two thin shells are drawn close together far from the camera
- **THEN** the nearer one is drawn in front consistently, without the two
  flickering against each other between frames

#### Scenario: The depth range does not pop
- **WHEN** the camera moves smoothly toward the subject
- **THEN** the derived near and far planes change smoothly with it

### Requirement: Materials remain correct at a distance
MatCap textures and reference images SHALL carry mip chains and SHALL be
sampled with mip filtering. MatCap mip levels SHALL be generated from the
material's own recipe at each level's size rather than by downsampling a
gamma-encoded image; reference image mip levels SHALL be filtered in linear
colour.

#### Scenario: A distant subtool does not sparkle
- **WHEN** a subtool is displayed small enough that its normals vary by more
  than a texel of the MatCap between neighbouring pixels
- **THEN** its shading is stable between frames as the camera moves, rather
  than aliasing against the MatCap's texels

#### Scenario: An obliquely viewed reference stays readable
- **WHEN** a reference image plane is viewed at a shallow angle
- **THEN** the image is filtered rather than aliased along the direction of
  greatest compression

### Requirement: Shading offers an optional presentation mode
The viewport SHALL offer a Studio shading mode with a small fixed light rig,
rendered through a high-dynamic-range target and tone mapped, selectable beside
MatCap. MatCap SHALL remain the default and SHALL NOT be replaced. Studio mode
MAY offer environment lighting and a single fitted directional shadow map; both
SHALL be optional and SHALL apply in Studio mode alone.

#### Scenario: MatCap stays the default
- **WHEN** the application starts with no stored preference
- **THEN** the sculpt is shaded with a MatCap, and no light rig, HDR target or
  shadow map is allocated

#### Scenario: Studio mode does not slow the sculpt path
- **WHEN** a stroke is made with Studio mode selected
- **THEN** the frames drawn during the stroke drop to the interactive quality
  tier as they do under any other profile

### Requirement: The pass order decides what occlusion reaches, and is stated
The viewport SHALL draw in a stated order: the opaque scene with the helpers
that lie behind or on it, the multisample resolve, the depth reduction, the
occlusion kernel, the depth-aware upsample multiplied onto the resolved colour,
and then the scaffolding — the lattice cage, an object's outline, the
manipulator and the orientation gizmo — onto that finished frame.

Occlusion SHALL therefore be applied through the depth the sculpt's surface
wrote. Where nothing was drawn it SHALL leave the frame alone, so the grid, the
symmetry planes and the reference planes are not darkened. The scaffolding SHALL
NOT be darkened by occlusion at all, whatever stands behind it: it is drawn
after the composite, because it stands *over* the form rather than on it, and a
manipulator dimmed by the fold it is being aimed at is dimmed exactly where a
sculptor is most likely to be aiming.

The scaffolding's own depth cue is not this. A comparison against the depth the
sculpt wrote is not occlusion, carries none of the occlusion field's shape, and
SHALL be identical whether occlusion is enabled or not.

Where the scaffolding is drawn faint, the form shows through it, and that form
SHALL be shaded as the form is. So the invariant is stated over the pixels the
scaffolding covers **opaquely**: those SHALL be identical under both occlusion
settings. A faint pixel is part widget and part sculpt, and the sculpt's share
of it darkens because it is the sculpt.

The orientation gizmo SHALL NOT be occluded by the sculpt. It is drawn in a
corner viewport with a camera of its own, and the scene's depth buffer says
nothing about it.

#### Scenario: The manipulator is not shaded by the form behind it
- **WHEN** a manipulator is drawn over an occluded fold, and the same frame is
  drawn with occlusion switched off
- **THEN** every pixel the manipulator covers opaquely is identical between the
  two

#### Scenario: A faint pixel shows the form, and the form is shaded
- **WHEN** the same two frames are compared where the manipulator is drawn faint
- **THEN** those pixels may differ, by as much as the form seen through them
  differs

#### Scenario: The sculpt beneath it still is
- **WHEN** the same two frames are compared over the surface rather than over
  the manipulator
- **THEN** the surface is darkened where it closes in on itself

#### Scenario: The orientation gizmo survives a sculpt in its corner
- **WHEN** the camera is close enough that the form fills the corner the
  orientation gizmo sits in
- **THEN** the gizmo is drawn

#### Scenario: The ground is not darkened
- **WHEN** a frame is drawn with occlusion on
- **THEN** the pixels the surface did not write depth at are unchanged

### Requirement: The scaffolding is drawn faint where the sculpt stands in front of it
The scaffolding SHALL be drawn at reduced strength where the sculpt's surface
is nearer to the camera than the scaffolding fragment, and at full strength
everywhere else. A rotate ring around a solid form SHALL therefore read as a
hoop the form passes through rather than as a circle painted on the frame.

This SHALL be done by **sampling** the depth the scene wrote, not by depth
testing: the pass SHALL bind no depth attachment and SHALL keep its place after
the occlusion composite. The value sampled SHALL be the frame's own reduced
depth — single-sampled, so the comparison does not fork on whether the device
multisamples.

Faint SHALL mean faint and nothing else. A dimmed handle SHALL remain drawn,
SHALL remain hit-testable on exactly the same terms as a bright one, and SHALL
NOT be hidden, since the hit test walks handles by ray and ignores depth.

Where the sculpt wrote no depth the scaffolding SHALL be drawn at full strength.
The surface is the only thing that writes depth, so scaffolding over empty
space, over the grid, over a symmetry plane, over a reference image, or over a
**ghosted** surface is unaffected — which is what keeps a deformation cage
"seen through, not turned off" while it is being edited.

The orientation gizmo SHALL NOT be dimmed. It draws in a corner viewport with a
camera of its own, and the scene's depth in those pixels is whatever a sculpt
reaching into that corner happened to write.

#### Scenario: A ring around a solid form
- **WHEN** a rotate manipulator encircles a solid form, so that part of the
  ring is behind it
- **THEN** the part behind the form is drawn fainter than the part in front,
  and both are still drawn

#### Scenario: Nothing behind it to be behind
- **WHEN** the same manipulator is drawn with no form present
- **THEN** every part of it is drawn at full strength

#### Scenario: A ghosted surface dims nothing
- **WHEN** the same manipulator is drawn against a ghosted surface, which
  writes no depth
- **THEN** every part of it is drawn at full strength

#### Scenario: The orientation gizmo is not dimmed by a sculpt in its corner
- **WHEN** the camera is close enough that the form fills the corner the
  orientation gizmo sits in
- **THEN** the gizmo is drawn at full strength

### Requirement: The depth the scaffolding reads does not depend on the occlusion setting
The depth reduction SHALL run when there is scaffolding to draw, whether or not
occlusion is enabled. The occlusion kernel and the composite SHALL still run
only for occlusion, and a frame with neither scaffolding nor occlusion SHALL run
neither.

A manipulator's appearance SHALL NOT depend on whether occlusion is on. Gating
the reduction on the occlusion setting would make it depend on exactly that.

#### Scenario: The same manipulator under both occlusion settings
- **WHEN** a manipulator standing partly behind a solid form is drawn with
  occlusion on and with occlusion off
- **THEN** the pixels it covers are the same in both, dimming included

