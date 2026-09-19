## 1. The rig, read back correctly

- [x] 1.1 Give `SkinSettings` the inverse of `radius_for`, written as one rather than as a division by `thickness`, so the clamp the multiply applies is the clamp the division undoes.
- [x] 1.2 Pass the thickness in hand into `read_armature` and `recover_armature`, and divide by it rather than by `SkinSettings::default()`.

## 2. A rig that comes back

- [x] 2.1 Re-examine every layer in `resync_armature`, not only the ones that still carry a rig: a layer whose rig was cleared on the way back is the one a redo has to put right.
- [x] 2.2 Put the sculptor on a rig a step brought back, since `armature()` answers for the active subtool alone. Only one that *arrived* with the step, so a sculptor working on another subtool is not moved.

## 3. A thickness that steps

- [x] 3.1 Note each thickness change against the stamp of the engine entry its rewrite left, the way a crossing is noted, and only where the rewrite actually left one.
- [x] 3.2 Carry the multiplier over that entry in both directions, before the rigs are re-read — what they are read through is the thickness the step restores.
- [x] 3.3 Drop the record when the engine drops the entry that names it, and clear the forward side when a new edit ends the redo line.
- [x] 3.4 Re-read the thickness in `ArmatureViewModel::refresh`, so the slider follows the step rather than describing a rig the document no longer has.

## 4. The curve in hand

- [x] 4.1 Add `resync_curve` beside `resync_armature` and call it from every path that steps the history.
- [x] 4.2 Rebuild the points from the guide the document holds, and set what the engine was last given so an append still names the end it changed.
- [x] 4.3 Empty the hand where the step went past the sweep's creation, and remember the node's id while it is gone — the engine hands the same one back, and forgetting it would place a second tube beside the first.
- [x] 4.4 Let go of a curve whose layer left the scene with the step.

## 5. Refusals instead of empty entries

- [x] 5.1 Refuse `set_radius` for a sphere that is not there, in the tree itself, so every caller meets the same answer.
- [x] 5.2 Refuse `reparent` for a sphere that is not there, beside the parent check that was already made.

## 6. Hold it

- [x] 6.1 `crates/clayspace-engine/tests/history_resync.rs`: a rig survives undo and redo past its own creation and is editable afterwards; radii round-trip five times at thickness 0.5; a thickness change is one undoable step.
- [x] 6.2 The same file: a curve's points follow a step, an undone point does not come back on the next edit, and a curve undone past its sweep comes back whole.
- [x] 6.3 `crates/clayspace-app/tests/armature_undo.rs`: the same three rig cases through the ViewModels the application drives, plus a no-op rig edit banking nothing.
- [x] 6.4 `crates/clayspace-vm/tests/armature.rs`: a resize and a reparent naming a sphere that is not there are refused out loud and leave the tree alone.
