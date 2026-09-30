## Design
The object ViewModel knows the active representation at `BeginGizmoDrag`. For a whole SDF layer it sets the existing pending/settling state immediately, so drag commands update the desired transform without calling the engine. The app sends the start and pending transforms to the renderer when exactly one layer is visible. The renderer computes the affine map `to * inverse(from)` and its inverse-transpose normal map once per frame, writes these to the camera uniform, and applies them only in the surface vertex and shadow passes. The grid and manipulator shaders continue reading original world coordinates.

On release, the app clears the renderer preview, while the ViewModel writes the final transform once and the app settles the SDF surface. This keeps the field authoritative and the existing drag history grouping intact.

Any unrelated command while a gizmo gesture is open ends that gesture first, then changes selection or mode. The object ViewModel does the same for direct dispatch and retains the gesture's original target for its final edit. `EndGizmoDrag` always clears the renderer preview, including when the final model edit is refused or the current selection has changed.

A combined multi-layer SDF cannot be transformed as one picture without also moving stationary layers. This path retains only the pending manipulator and releases to the exact combined field. A future upstream layer placement preview cache could improve that case.

## Measurement
On the reference scene in a debug build, the former first drag spent 49 ms in the engine edit and 241 ms in the full surface settle. An attempted CPU retained-mesh preview still took 101 ms because it reuploaded many brick spans. The renderer uniform update measured below 0.1 ms. A real GPU offscreen frame with readback took 82.6 ms before the drag and 73.4 ms with the preview; readback dominates these figures, so they establish that the preview adds no measured frame cost in this harness rather than a 60 Hz runtime guarantee. The preview and released frames differed by 0.0101 mean normalized pixel value on the reference translation; the regression allows 0.04 for re-sampling differences.

## Placed objects
A placed object in a field cannot be previewed by moving the retained surface: the surface is the composition of every item in the layer, and only one of them is moving. The drag draws the moving item on its own instead.

- **The object's surface, once per drag.** `ObjectModel::object_preview` builds a scratch ClayCore document holding one item of the object's primitive at the origin and meshes it at 48 cells across its largest extent with surface nets. The real document and its brick cache are not read or written. A mesh sampled into the field has no primitive to mesh and returns `None`, and its drag stays live.
- **Posed on the CPU each frame.** `ObjectPreview::posed` takes the manipulator's world transform back into the subtool's frame with the same `unplace` the document uses when the drag is written, then places each vertex as the engine places the item: node transform, then the mirror reflection, then the subtool's transform. Normals use the inverse-transpose map. A reflected image has its winding reversed so its front faces still face out. For a sphere of 21,468 triangles and its X twin, posing and uploading measured 0.54–0.75 ms a frame; meshing at the press, 1.9–6.6 ms. This avoids a per-instance uniform and a second bind group, which would also have to be rebuilt whenever the MatCap changes.
- **Mirror participation is recorded.** The engine emits one reflected copy per set mirror axis for an item that follows the layer's mirror, and the ABI has no reader for an item's participation. `PlacedObject::mirrored` records it when the item is built (off only for an object placed with symmetry off). The side-car appends it after the per-axis scale, so an older build reads the row as before. A row without it cannot be resolved: builds since #277 already placed objects made with symmetry off out of the mirror, and nothing reads that back. It is read as mirrored, the common case; a one-sided object from such a file draws a twin during its drag that the release does not keep. The layer's own mirror is asked of the engine, not of the host's record, which a reopen resets to "no mirror".
- **Only union objects.** The ViewModel asks for a preview only when the object's operation is `Add`. Every other operation of the thirteen keeps the live path and its adaptive deferral, including Tongue, Emboss and Pipe, which also add material: each is shown by what it does to the form, not by its bare shape. `object-transform`'s "A live operand stays interactive while it is dragged" is modified by this change to say so; before it, it covered union objects too, whose blend was drawn live wherever the frame allowed.
- **A hidden layer draws nothing.** The app draws the posed preview only when the object's layer is visible (`object_drag_frame` in `main.rs`); a hidden layer's object otherwise appeared mid-drag as the only shaded thing of its layer.
- **Drawn with the surface.** The renderer draws the posed triangles after the field with the surface pipeline and bindings, so they take the same material, depth and occlusion. They cast no Studio shadow. `EndGizmoDrag` clears the preview before the settle, including an end caused by an interrupting command.

## Measurement (placed object)
`gizmo_first_drag.rs::the_first_object_drag_frame_draws_the_object_alone` builds the reference scene twice with a 0.3 sphere placed on its flank, under the default X symmetry. It times main's first frame on one copy (the edit, then `SurfaceGeometry::sync`) and the previewed press and first frame on the other. Release build, Metal, under heavy parallel build load (load average up to 144), nine runs:

| first drag frame | main | this change |
|---|---|---|
| document edit | 16.5–43.7 ms | none |
| surface sync | 24.0–68.3 ms (390 keys) | none (`sync` returns `None`) |
| object meshed at the press, posed and uploaded | — | 2.4–7.4 ms (42,936 triangles, two images) |
| total | 45.1–98.0 ms | 2.4–7.4 ms |

The same probe on a subtracting sphere measured 46–56 ms (286 keys) on main. That path is unchanged here.

On release, the previewed document's surface matches the live path's at the quantised level on Metal and bit for bit on the CPU backend (349,744 triangles). Metal alone is not bit-reproducible: two identical live drags differed in the last bits of 7–77 triangles from run to run, all within 1/4096. The preview frame differs from the released frame by 0.46 mean levels, because the object's old image is still in the field.
