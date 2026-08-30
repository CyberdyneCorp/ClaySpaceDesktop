# Tasks

## 1. An undo costs the edit
- [x] 1.1 Wrap `clay_document_undo_bound` / `_redo_bound`, reusing `Influence`
  for the three states the header says they share
- [x] 1.2 Re-mesh what the step reached instead of the active layer's bound
- [x] 1.3 Measure before and after on the same fixture, moments apart
- [x] 1.4 Hold the new claim in `undo_cost.rs`, replacing the assertion that
  encoded the whole-layer bound — mutation-checked
- [x] 1.5 `undo_region.rs`: an undo on another subtool re-meshes that subtool

## 2. A costly subtool says so
- [x] 2.1 Measure why nothing asked: the advice is 33 µs and the byte estimate
  beside it is 287 ms
- [x] 2.2 Carry the cheap half in the scene as `FieldHealth`
- [x] 2.3 Offer the collapse in the subtool panel, only while the engine
  advises it, and act only when the sculptor asks
- [x] 2.4 `field_health.rs`: the advice arrives, collapsing clears it, and the
  scene is not asking what collapsing costs
- [x] 2.5 `visual_shell.rs`: the offer is drawn, and only when it is being made

## 3. Gates
- [x] 3.1 The full suite, fmt, clippy, layering, openspec and packaging
