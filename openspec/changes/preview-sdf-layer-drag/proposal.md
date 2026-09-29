## Why
The first manipulator move after selecting a whole SDF subtool blocks the UI while the engine refills the field and the viewport fully re-meshes it. On the reference scene, the edit cost 49 ms and the settle cost 241 ms in a debug profile. The widget's existing adaptive deferral starts only after that first slow frame.

## What Changes
- Defer whole SDF layer evaluation from the first drag frame until release.
- Transform the retained surface in the renderer during a drag when it is the only visible layer.
- Clear the preview before applying the final transform and rebuilding the exact SDF surface.
- Keep the current live path for placed objects. In multi-layer scenes, the widget follows the drag while the combined SDF picture waits for release because moving the composite would incorrectly move other layers.

## Impact
The first drag frame needs only a uniform update; no engine edit or mesh upload. A single-layer preview is affine and may differ slightly from the finally re-sampled SDF, especially after a large nonuniform scale. The final surface is authoritative. Multi-layer drags have no clay preview during the gesture.
