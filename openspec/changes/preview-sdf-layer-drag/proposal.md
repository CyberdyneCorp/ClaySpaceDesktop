## Why
The first manipulator move after selecting a whole SDF subtool blocks the UI while the engine refills the field and the viewport fully re-meshes it. On the reference scene, the edit cost 49 ms and the settle cost 241 ms in a debug profile. The widget's existing adaptive deferral starts only after that first slow frame.

The same first frame stalls for a placed object in a field. After the layer preview and #312, the first drag frame of a placed sphere on the reference scene still measured 45–98 ms in a release build (a 16–44 ms edit, then 24–68 ms re-meshing the 390 bricks its old and new bounds reached) before the deferral took over (#196, D14). A layer-wide affine preview cannot help: the object is one item blended into a field whose other items must stay where they are.

## What Changes
- Defer whole SDF layer evaluation from the first drag frame until release.
- Transform the retained surface in the renderer during a drag when it is the only visible layer.
- Clear the preview before applying the final transform and rebuilding the exact SDF surface.
- In multi-layer scenes, the widget follows the drag while the combined SDF picture waits for release because moving the composite would incorrectly move other layers.
- Draw a placed object that adds material as its own surface during its drag, and write the field once on release. The object's primitive is meshed alone at the press and posed on the CPU each frame, with one reflected image per mirror axis it takes part in. The object table records whether each object takes part in its layer's mirror, and the side-car stores it.
- Keep the live path for operands that subtract, intersect, groove or paint, as `object-transform` requires ("A live operand stays interactive while it is dragged").

## Impact
The first drag frame needs only a uniform update; no engine edit or mesh upload. A single-layer preview is affine and may differ slightly from the finally re-sampled SDF, especially after a large nonuniform scale. The final surface is authoritative. Multi-layer drags have no clay preview during the gesture.

A placed object's first drag frame poses and uploads its own triangles (2.4–7.4 ms measured, press included) instead of refilling and re-meshing the field. The preview is the bare primitive: it does not show the blend into its neighbours, and the object's old image stays in the field until release. The released surface is the live path's; on the CPU backend it is bit-identical. Subtractive and intersecting operands still pay one live evaluation on their first frame (46–56 ms on the reference scene).
