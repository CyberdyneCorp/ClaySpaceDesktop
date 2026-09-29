## 1. Implementation
- [x] Defer SDF layer evaluation from the first drag frame.
- [x] Render single-layer surface preview with a GPU affine transform.
- [x] Clear the preview and settle the document on release.

## 2. Verification
- [x] Compare preview and committed pixels in a regression test.
- [x] Profile the first frame and document the measured improvement.
- [x] Run relevant ViewModel, renderer, and app tests plus OpenSpec validation.
