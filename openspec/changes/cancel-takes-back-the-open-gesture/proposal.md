# Cancel takes back the open gesture

Cancelling a stroke is the safe way out of a gesture: the sculptor presses Esc
and the clay is where it was before they pressed the pointer down. On a mesh
layer it was the most destructive command in the application.

Reproduced three times: cancel erased gestures committed **before** the one
being cancelled; cancel blanked the mesh entirely, the inspector reporting 0
vertices and 0 faces; and on a layer created a moment earlier, cancel deleted
the layer.

## What went wrong

The sculpting ViewModel counted the entries a gesture produced and spent that
many undos to take it back. The count is one per applied segment, which is what
the **field** path records: each segment is its own engine entry, and a gesture
drawn in twenty segments is twenty entries.

A mesh gesture is not recorded that way and deliberately is not. It is
previewed while it is made — the model takes back what the last segment did and
lays the whole gesture down again from its anchor — and the release banks the
whole thing as **one** record, so that one drag stays one undo and the
per-segment cost stays affordable. So on a mesh the count said twenty and the
gesture was one: the first undo took the gesture back, and the nineteen after
it took back whatever was underneath. The gestures already committed, and on a
layer nothing else stood under, the layer itself.

Two smaller things sat in the same code:

- **A cancel with nothing open reported success and reverted anyway.** The
  count left over from whatever came last was spent on history the gesture
  never made, and the caller was told it had worked.
- **A cancel with nothing open still ended a gesture on the model**, settling
  the document a second time for no reason. A cancel an agent may safely repeat
  — the interface sends a release whether or not the press opened anything —
  must cost nothing at all.

## What this changes

The ViewModel reads the model's history depth when the gesture opens and
reverts down to it, never past it. A depth is the wrong thing to make an
*ordering* from and the document has stamps for that, but "how far has this
gesture got" is exactly what a depth answers, and it answers it the same way
whatever the representation wrote above the line: one record, or twenty.

- **Cancel is bounded by its own gesture.** What was committed before the press
  stands, and so does the layer.
- **A cancel with nothing open is a no-op that says so.** It is not a refusal —
  it must stay repeatable — so it reports that there was nothing to cancel.
- **Cancel says what it did.** It reports itself rather than leaving the last
  applied segment standing as the last action, which read as the stroke having
  landed.

## What this deliberately does not do

**It does not make the mesh path bank one entry per segment.** That would make
the count right by making the gesture expensive, multiplying the per-segment
cost of a mesh stroke and undoing the batching that makes mesh gestures
affordable at all.

**It does not change what a commit banks.** The release still banks one action
on the history the interface reads, and Undo still spends the count that action
recorded.

**It does not make cancel fast.** The refill a cancel triggers on a large mesh
is the same refill an undo triggers, and it is tracked with the rest of the
work that belongs off the interface thread.
