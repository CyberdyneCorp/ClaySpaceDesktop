# A rig and a curve survive a history step

Two tools hold their subject on this side of the ABI while the engine holds
what it produces. A rig is a tree of spheres here and a set of scaled radii
there; a curve is a list of control points here and a swept guide there. Undo
moves the engine's half and has no way to say so, so both have to be read back
after every step. The rig was — partly, and through the wrong number. The
curve was not read back at all.

## What went wrong

**A rig could not be brought back.** `resync_armature` re-read only the layers
that still carried a rig. Stepping back past a rig's creation correctly cleared
the record, and then nothing ever looked at that layer again: stepping forward
left the armature in the document and out of the editor's reach, an "Armadura"
subtool refusing every edit over a surface that plainly had a skeleton in it.

**Undo at a thickness other than 1 baked the thickness into the radii.** The
tree keeps the radii a sculptor authored and the document keeps them with the
thickness applied, so reading one back is a division — and it divided by
`SkinSettings::default()` whatever the slider said. Measured at thickness 0.5:
a rig came back halved, and each cycle halved it again. It was not undoable and
it was not recoverable.

**The thickness itself was not undoable.** It is a multiplier the engine never
sees as one: the radii reach it already scaled. So the rewrite a change forces
is an ordinary engine entry that undo reverts exactly, while the multiplier
stayed where the slider left it — the old radii under the new thickness, which
is the same wrong division by another road.

**A curve's points went out of step with its guide.** Every control point
reaches the engine as a guide the moment there are two of them, so undo takes
points back. The hand kept them, the panel offered a point the surface no
longer had, and the next point was appended past it — which wrote the undone
one straight back out.

**A rig edit on a sphere that is not there still cost an undo step.** `resize`
and `reparent` fell through their guards and changed nothing, while the rewrite
above them placed the tree again regardless. The sculptor's next ⌘Z was spent
on a gesture that never happened.

## What this changes

- **Every layer is re-examined after a step**, not only the ones that still
  hold a rig, so a redone creation comes back editable. A rig that arrives with
  a step takes the sculptor with it, the way a reopened document puts you on
  the first rigged subtool — `armature()` answers for the active subtool alone,
  so a rig nobody is standing on is a rig nobody can edit.
- **Radii are read back through the live thickness**, and through the same
  clamp the multiply uses, so the round trip is exact at any setting.
- **A thickness change is noted against the engine entry its rewrite left**, so
  a step over that entry carries the multiplier with it. The same shape as a
  crossing, and for the same reason: the engine names the entry, the document
  remembers what the entry meant on this side.
- **The curve in hand is rebuilt from the document after every step**, or
  emptied where the step went past the sweep's own creation. The sweep's node
  id is remembered while history has it taken back, because the engine hands
  the same one back on the way forward and a curve that forgot it would place a
  second tube beside the first.
- **An edit naming a sphere that is not there is refused** in the tree itself,
  before anything reaches the document.

## What this deliberately does not do

**It does not make the thickness per rig.** It is still one multiplier for the
document, so two rigs in one document share it and reading either back uses
the one in hand. That is a separate defect with a separate fix, and this change
only makes the multiplier behave under history.

**It does not restore the active subtool for anything but a rig.** A layer that
a redo brings back does not take the selection with it in general; the rule
added here is about a rig arriving, which is the case where the selection
decides whether the tool works at all.
