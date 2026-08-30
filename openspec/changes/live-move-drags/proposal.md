# Drag with Move without steepening the field once per segment

## Why

Move degrades an SDF layer by a mechanism the smoothing work did not touch, and
`clay_field_report` names it separately for exactly that reason. A drag warps
every item it reaches with a `grab` deformer, and the engine's Lipschitz bound
for a chain is the **product** of its links — so the safe step scale decays by a
constant factor per grab and the marcher's cost rises geometrically. ClayCore's
own policy header states it: *"each drag appends a `grab` to the deformer chain
and deformer_lipschitz multiplies them, so the safe step scale decays by a
constant factor per drag, 79x the marching cost by nine."*

The application made that worse than it had to be. A drag arrives in segments,
and each segment called `clay_layer_move_surface` — so the field paid per
**segment**, not per gesture. Measured on the starting form, twelve drags of six
segments each:

| delivery | deformer chain | safe step scale |
|---|---|---|
| one grab per segment | 72 | 0.000608 |
| one grab per gesture | 12 | 0.002456 |

and a segment went from 5.2 ms to 26 ms across those twelve drags while the
document held nothing a sculptor could see for it.

There is a second, quieter fault in the same place. A segment computed its
displacement from its own first sample, so each one anchored where the last
stopped and the drag **composed** rather than replaced. The engine is explicit
that this is wrong: *"Updates of 0.10, 0.20, 0.50 must end at exactly what a
single fresh drag of 0.50 produces, not at a composition of three warps each
authored against a different intermediate surface."*

`clay_sdf_move_*` answers both. The edit list is walked once at pointer-down,
every frame after costs only the items the drag moves, an update takes the total
measured from the anchor, and the commit rebuilds one chain per item as one undo
step. The wrapper has been carried since `live-field-brushes` (task 2.2) and
nothing has used it.

## What changes

- Move on a field is driven by `clay_sdf_move_*`: begun on the gesture's first
  segment, updated with the total displacement, committed when the pointer comes
  up. The old per-segment `clay_layer_move_surface` path remains for a layer the
  transaction refuses.
- The drag is **previewed** while it is made, so nothing about how Move feels
  changes.
- One drag is one history entry, as it was.

## What this does not fix

A session of drags still compounds: twelve gestures are twelve links, and the
factor per link is the engine's. The engine offers `clay_sculpt_policy`'s
`max_deformer_chain` with `allow_consolidation` to collapse the layer inside the
stroke's own undo step, and it is **not** taken here, because it measures worse:
the same gesture after a collapse costs 1345 ms against 211 ms on the parametric
layer, even though the collapse *improves* the safe step scale from 0.00275 to
0.08090. A collapsed layer is one dense volume, and every verb that re-samples
or warps it pays per sample what it used to pay per primitive. Recorded under
*Known costs and escape routes* in `docs/roadmap.md` rather than guessed at.

## The preview, and the ABI gap behind it

ClayCore's C++ `SdfMoveTransaction` exposes `preview_layer()` — a copy of the
layer with the affected chains replaced — and the sdf-sculpt-transaction spec
names it as how a host draws a Move preview: *"so it compiles, draws and picks
like any other layer."* **`clay.h` does not carry it.** This application can
only reach the C ABI, so the preview takes the other route the header offers:
the resolved grabs, *"so a host can reproduce the preview through machinery it
already has."*

Once per segment the application writes those grabs onto the layer, lets the
brick cache sample the dragged surface out of the document, and undoes them
inside the same segment. Three facts make that sound, and all three are asserted
rather than assumed:

1. a written grab moves the surface, so there is something to look at;
2. each is one undo entry, so the application spends exactly as many as it
   wrote — the ABI has no remove-deformer call, and undo is the only door;
3. a commit accepts a layer that was edited and restored, because its stamp is
   derived from **content** rather than from a counter.

Undoing inside the segment is also what keeps the history honest: the ViewModel
counts a live segment by the undo depth it left behind, so a segment that kept
its preview would be counted as having written it, and cancelling would then
spend one undo per segment against history the gesture never made.
