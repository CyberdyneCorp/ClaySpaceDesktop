## Design
The object ViewModel knows the active representation at `BeginGizmoDrag`. For a whole SDF layer it sets the existing pending/settling state immediately, so drag commands update the desired transform without calling the engine. The app sends the start and pending transforms to the renderer when exactly one layer is visible. The renderer computes the affine map `to * inverse(from)` and its inverse-transpose normal map once per frame, writes these to the camera uniform, and applies them only in the surface vertex and shadow passes. The grid and manipulator shaders continue reading original world coordinates.

On release, the app clears the renderer preview, while the ViewModel writes the final transform once and the app settles the SDF surface. This keeps the field authoritative and the existing drag history grouping intact.

Any unrelated command while a gizmo gesture is open ends that gesture first, then changes selection or mode. The object ViewModel does the same for direct dispatch and retains the gesture's original target for its final edit. `EndGizmoDrag` always clears the renderer preview, including when the final model edit is refused or the current selection has changed.

A combined multi-layer SDF cannot be transformed as one picture without also moving stationary layers. This path retains only the pending manipulator and releases to the exact combined field. A future upstream layer placement preview cache could improve that case.

## Measurement
On the reference scene in a debug build, the former first drag spent 49 ms in the engine edit and 241 ms in the full surface settle. An attempted CPU retained-mesh preview still took 101 ms because it reuploaded many brick spans. The renderer uniform update measured below 0.1 ms. A real GPU offscreen frame with readback took 82.6 ms before the drag and 73.4 ms with the preview; readback dominates these figures, so they establish that the preview adds no measured frame cost in this harness rather than a 60 Hz runtime guarantee. The preview and released frames differed by 0.0101 mean normalized pixel value on the reference translation; the regression allows 0.04 for re-sampling differences.
