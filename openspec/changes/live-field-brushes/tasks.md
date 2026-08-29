# Tasks

## 1. Decide the shape
- [x] 1.1 Establish whether the preview can be drawn without a second
  implementation of the field — routes 1 and 2 of `06-host-gpu-previews.md`
- [x] 1.2 Measure the preview lattice against the brick cache's: spacing, halo
  width and origin alignment, on an off-lattice form
- [x] 1.3 Measure the snapshot path per frame and reject it with the number
- [x] 1.4 Correct `docs/architecture.md`, which stated the constraint too broadly

## 2. The wrapper
- [x] 2.1 `SmoothTransaction` — begin, update, preview delta, preview item,
  commit, cancel, and a `Drop` that cancels
- [x] 2.2 `MoveTransaction` — begin, update, reached nodes, resolved grabs,
  commit, cancel
- [x] 2.3 `BrickCache::submit`, which stores samples the caller produced, and
  `BrickSubmit` — the outcome `refill` used to discard, so a cache refusing
  every brick for want of budget is no longer invisible

## 3. The gesture
- [x] 3.1 Relabel the delta's bricks into a cache of the preview's own lattice
- [x] 3.2 Prime the preview with one zero-strength whole-volume pass, so the
  first frame draws the whole form rather than the dabbed patch
- [x] 3.3 Drain the dirty pool to the bottom: marking a brick dirties its
  neighbours, and a bounded take left most of the volume unsubmitted
- [x] 3.4 Point the layer's mirror *before* the transaction opens, and account
  for the entry that records
- [x] 3.5 Open, dab, commit and discard on `ClayDocument`; the ViewModel stops
  holding the gesture when the model says it is live
- [x] 3.6 Draw from the preview's cache, translated, and lay the surface out
  again when it swaps

## 4. Gates
- [x] 4.1 The preview shows itself before the document changes
- [x] 4.2 What the preview showed is what the commit installs — mutation-checked
  against a shifted lattice
- [x] 4.3 An abandoned gesture leaves the document as it was
- [x] 4.4 A second field subtool falls back to the held gesture
- [x] 4.5 Through the pointer, not just the document: a stroke shows itself
  before it ends
- [x] 4.6 A stamping drag on a mesh builds up rather than replacing itself,
  and is still one undo — the fault forwarding the gesture hooks uncovered
- [x] 4.7 A structural check that every *provided* method of every model trait
  is forwarded, so this class of fault cannot be reintroduced by omission
- [x] 4.8 The roughened-surface gate reproduces the old numbers exactly, on
  every backend — the transaction drives the preview and not the edit
- [x] 4.9 The full suite, fmt, clippy, layering, openspec and packaging
