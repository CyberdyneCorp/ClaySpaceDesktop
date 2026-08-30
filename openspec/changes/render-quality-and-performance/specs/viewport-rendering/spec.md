## ADDED Requirements

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
NOT be darkened at all, whatever stands behind it: it is drawn after the
composite, because it stands *over* the form rather than on it, and a
manipulator dimmed by the fold it is being aimed at is dimmed exactly where a
sculptor is most likely to be aiming.

The orientation gizmo SHALL NOT be occluded by the sculpt. It is drawn in a
corner viewport with a camera of its own, and the scene's depth buffer says
nothing about it.

#### Scenario: The manipulator is not shaded by the form behind it
- **WHEN** a manipulator is drawn over an occluded fold, and the same frame is
  drawn with occlusion switched off
- **THEN** every pixel the manipulator covers is identical between the two

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

## MODIFIED Requirements

### Requirement: The viewport renders geometry produced by the engine's meshers
The viewport SHALL display triangles produced by ClayCore's meshers. It SHALL use the surface-nets preview mesher for interactive display and marching tetrahedra where a watertight, 2-manifold result is required. The application SHALL NOT reimplement any distance function, combine operator, blend profile or deformer in a shading language.

This holds for every shader this renderer gains — occlusion, depth reduction,
bilateral upsample, cavity, tone mapping, studio lighting and shadowing are
display code over a depth buffer and a mesh, and none of them evaluates the
field that produced either.

#### Scenario: No field math in shaders
- **WHEN** the WGSL shader sources are inspected
- **THEN** they contain shading, transform and display code only, and no signed-distance primitive, blend or deformer evaluation

#### Scenario: The displayed surface is the document's surface
- **WHEN** a document is displayed in the viewport and separately meshed through the engine at the same resolution
- **THEN** the two surfaces are the same geometry, because the viewport did not compute one of its own

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
