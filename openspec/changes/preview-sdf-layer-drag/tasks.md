## 1. Implementation
- [x] Defer SDF layer evaluation from the first drag frame.
- [x] Render single-layer surface preview with a GPU affine transform.
- [x] Clear the preview and settle the document on release.

## 2. Verification
- [x] Compare preview and committed pixels in a regression test.
- [x] Profile the first frame and document the measured improvement.
- [x] Run relevant ViewModel, renderer, and app tests plus OpenSpec validation.

## 3. Placed objects
- [x] 3.1 Mesh a placed object's primitive alone at the press (`ObjectModel::object_preview`) and pose it per frame with its mirror images (`ObjectPreview::posed`).
- [x] 3.2 Record mirror participation in the object table and the side-car; old rows are read as mirrored, the common case; the layer's mirror is asked of the engine, so it survives a reopen.
- [x] 3.3 Defer the field edit of a union object's drag to release and draw the posed preview; keep every other operation live; draw nothing while the object's layer is hidden.
- [x] 3.4 Regression tests: the ViewModel defers and writes once (`objects.rs`); the engine's preview is the object, placed and reflected as the engine places it (`object_drag_preview.rs`); on the reference scene the first frame re-meshes nothing and fits the frame, nothing but the object moves on screen, and the release matches the live path, bit for bit on the CPU backend (`gizmo_first_drag.rs`).
- [x] 3.5 Review fixes: the reopened document's preview reflects what the engine does (`object_drag_preview.rs::a_reopened_document_previews_the_mirror_the_engine_evaluates`); the on-screen check boxes each image on its own and requires the twin drawn; the app's object-drag decision and release rule are unit-tested in `main.rs`; `object-transform` is modified to match.
