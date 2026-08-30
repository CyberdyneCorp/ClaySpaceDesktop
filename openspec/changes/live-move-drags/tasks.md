# Tasks

## 1. Establish the ground
- [x] 1.1 Measure the two degradation mechanisms apart — a chain of bakes
  against a chain of grabs — and confirm which one Move is
- [x] 1.2 Measure a segmented drag against a transactional one: chain length and
  safe step scale over twelve gestures
- [x] 1.3 Measure whether consolidation is an escape route for Move, and record
  that it is not

## 2. The one wrapper the preview needs
- [x] 2.1 `Document::add_grab` — `clay_layer_add_deformer` with the engine's own
  `CLAY_DEFORM_GRAB` parameter order, at the front of the chain
- [x] 2.2 Assert the three facts the preview rests on: a written grab moves the
  surface, each is one undo entry, and a commit accepts a layer edited and
  restored

## 3. The gesture
- [x] 3.1 `LiveMove` — begin at the anchor, drag to a position, settle, commit,
  cancel
- [x] 3.2 Arm the gesture at pointer-down and begin the transaction on the first
  segment, which is the first thing carrying a position
- [x] 3.3 Draw, sample, take back — inside one segment, so a segment leaves the
  engine's undo depth where it found it
- [x] 3.4 Re-fill the union of where the last preview stood and where this one
  does, so the clay a drag moves off is restored as it moves
- [x] 3.5 Do not reflect the gesture on the live path: `clay_sdf_move_*` resolves
  one grab per image of the drag already

## 4. Hold it there
- [x] 4.1 `live_move.rs` — the drag shows itself, the document stays clean, the
  result lands where the preview showed it, a whole drag is one grab and one
  entry, and abandoning one leaves neither a mark nor a preview
- [x] 4.2 `live_stroke.rs` — the same through the ViewModel, which is what
  decides to send segments at all
- [x] 4.3 `move_mirror.rs` — a mirrored live drag pulls each side once, by the
  other mirror mechanism
- [x] 4.4 `docs/features.md` and `docs/roadmap.md`, including the ABI gap and
  what is still not fixed
