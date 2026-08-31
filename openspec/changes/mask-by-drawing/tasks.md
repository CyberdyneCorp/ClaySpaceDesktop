# Tasks

## 1. Establish what the mask can be written with
- [x] 1.1 Measure a region delivered as many small writes against one stroke:
  4 ms a call on a document mask against 7 µs on a standalone one, and where
  that cost comes from — the lazy snapshot every mask entry point brackets
- [x] 1.2 Measure what the footprint and the pitch each decide: the pitch buys
  the region's edge and nothing else, and the footprint has to reach half the
  pitch's diagonal because the lattice is the camera's and the footprint is the
  world's — a ball at that reach against a cube is 800 ms against 1191
- [x] 1.3 Confirm a stamp run toward 0 releases what one toward 1 froze, so both
  halves of the gesture are one mechanism

## 2. The outline, and the region it stands for
- [x] 2.1 `clayspace-model/src/lasso.rs` — the frame, the outline, the two
  modes, the gesture, and the draft the pointer builds
- [x] 2.2 Containment by the even-odd rule, so a loop drawn back over itself
  leaves the overlap alone
- [x] 2.3 `coverage_path` — a depth-first walk of the columns the outline
  encloses, entering each at the end the walk stands at and leaving from the
  other, so a connector is one short segment rather than a jump
- [x] 2.4 `lattice_pitch` and `cells_to_write` — the pitch, fixed, and the
  estimate a region too large to freeze at once is refused against
- [x] 2.5 Hold the property everything rests on: the path never leaves the
  outline, and it reaches every column inside it

## 3. Through the mask to the engine
- [x] 3.1 `MaskModel::apply_lasso`, beside the operations that act on a mask
  that already exists
- [x] 3.2 `ClayDocument::apply_lasso` — the subtool's bounds grown by the
  footprint, the path, and one `clay_mask_apply_stroke` with a hard-edged cube
- [x] 3.3 The mask revision, since a lasso dirties no brick
- [x] 3.4 `mask_outline.rs` — the enclosed side resists, the far surface freezes
  with the near one, the modifier releases, a C is not its bounding box, a whole
  gesture is one undo, and one that missed says nothing

## 4. The gesture
- [x] 4.1 `SetMaskGesture`, `BeginMaskLasso`, `ExtendMaskLasso`, `EndMaskLasso`,
  `CancelMaskLasso`, and which of them touch the document
- [x] 4.2 The draft in `MaskViewModel`, so the overlay is read from state a
  headless test can assert on
- [x] 4.3 The frame carried in on `EndMaskLasso`, so the ViewModel never needs a
  camera
- [x] 4.4 `input::ndc_at` — one mapping shared by the ray, the outline and the
  overlay
- [x] 4.5 `Drag::Lasso`, armed before the surface is asked about, so a press
  beside the form draws rather than orbits
- [x] 4.6 The frame built from the camera's own rays through the subtool's
  centre

## 5. What it looks like
- [x] 5.1 `shell::lasso_overlay` — the line, the closing edge, and the two modes
  told apart by tint
- [x] 5.2 The Pincel/Laço/Retângulo control on the options bar, with the mask
  brush in hand, and the same three in the Máscaras menu — beside the brush's
  own numbers rather than at the end of the bar, where a narrow window pushed
  the last chip off the screen
- [x] 5.2a A rectangle is drawn as four equal edges and a lasso's closing edge
  faint, because only one of them closes across a gap
- [x] 5.2b The brush ring goes off with a drawn gesture in hand, through
  `input::shows_the_brush_ring` — the rule the cage already answers, asked by
  the press and by the ring from one place
- [x] 5.3 The three locales
- [x] 5.4 `visual_mask_outline.rs` — the outline over the form, the frozen
  region after it lands, and a dragged box landing where a traced outline round
  the same region does

## 6. Say so
- [x] 6.1 `docs/features.md` — the gesture, what it freezes, and the edge it
  gives
- [x] 6.2 `docs/roadmap.md` — the engine has no bulk mask write, what that
  costs, and what it would take to close
- [x] 6.3 `mask.outline` in the benchmark. **Not** spliced into the Linux
  baseline: that file was recorded against ClayCore 0.52.2 and `bench-compare`
  is already red on `main` without this change — verified by running it on a
  clean checkout — so a figure added to it now would be recorded against a
  comparison nobody can read. It goes in when the baseline is re-recorded for
  0.60.0, which `upgrade-engine-0-60-0` is for. A figure the baseline lacks is
  reported as `new` and does not fail the gate.
