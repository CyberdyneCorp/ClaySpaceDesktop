# An eye costs what it moves

Clicking the eye on a subtool froze the window. `layer.set_visible` stalled on
**66% of all calls**, p95 **19.7 s**, worst **82.3 s**; a solo reached
**41.8 s**; a grid display change over a document with six hidden grids took
**13.8 s**. Hiding a ~72–144k-triangle field cost 136–1,490 ms. Nothing in the
document changed: the sculptor said which subtools to look at.

## Why it happened

Every visibility write refilled a layer's bricks, and the refill drained
synchronously on the interface thread.

Three separate things were being paid for, and only one of them is real work:

- **A layer the field does not hold was refilled anyway.** The brick cache
  evaluates the fold of the *visible SDF* layers and nothing else. A grid and a
  carried mesh arrive by the other route entirely — `visible_mesh_geometry`
  assembles them into one buffer and selects which spans to hand over by the
  same `visible` flag — so their eye is honoured where the frame is assembled
  and there is nothing to re-evaluate. `write_layer_visible` marked and drained
  for them regardless.
- **A hop back through the history refilled the wrong layer.**
  `after_visibility_history` refilled the **active** layer, on the reasoning
  that a hidden layer contributes nothing to the field so the bound is the
  whole layer. That is true of the layer whose eye moved and false of the one
  that happens to be selected. Undoing a solo gave three subtools back to the
  document and one of them back to the cache: the stack showed four subtools
  and the viewport drew two. This was a **correctness** defect, not only a cost
  one, and nothing was holding it.
- **A display change re-meshed hidden grids.** `resmooth_voxels` rebuilt the
  smooth surface of every grid in the document. A whole-grid smooth mesh is 17
  to 21 ms, so six hidden grids were over a tenth of a second of interface
  thread spent on a picture none of them is in.

## What changes

- **A visibility write marks only a layer the field holds.** `Layer` answers
  the question once, and both the write path and the history hop ask it.
- **A hop refills exactly the layers the gesture wrote.** `VisibilityGesture`
  carries them — the layers it actually wrote a flag on, not the pattern it was
  asked for, since a flag already at the wanted value is left alone. They are
  marked together and drained once, as every other multi-layer operation is.
- **`resmooth_voxels` skips hidden grids**, and reports how many it rebuilt so
  a test can hold it to that without measuring a shared machine's clock. The
  work is not dropped, it is deferred to the frame that draws the grid — which
  is the frame that needs it.

## What this deliberately does not do

**It does not stop a field subtool's eye from refilling the field.** Hiding an
SDF layer genuinely changes the field the brick cache holds: the bricks it
reached have to be rewritten or the viewport draws a surface the document no
longer describes. The issue proposed marking the layer as not-drawn and
dropping its GPU buffers instead; there are no per-layer field buffers to drop,
because the cache holds one merged field addressed in world bricks. So hiding a
*worked* field subtool still costs its refill.

**It does not batch that refill across frames.** Bringing a worked field
subtool's toggle under the 16 ms budget needs the drain to stop being
synchronous on the interface thread, which is the same mechanism the slow
undos need. That belongs with the refill-batching work and not here: doing it
in this change would mean two different drains during the transition, and the
one that can be got right first is the one that names what to refill.
