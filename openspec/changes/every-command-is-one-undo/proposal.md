# Every command that changes the document is one undo

`edit-history` has required since the beginning that every operation changing
document state be undoable. The engine holds up its half: applying a cage,
running a deformer, adding a layer, inserting a shape, consolidating a layer
and resolving a boolean all record entries on its stack.

The history a sculptor presses is not the engine's. It is the sculpting
ViewModel's — **a stack of how many engine entries each action spent**, where
one Cmd+Z pops one count and undoes exactly that many. A stroke pushes its
count, an armature gesture banks its delta, a crossing and a grid repair were
given theirs in `#149`, the mask's in `#152`. The structural and deformation
commands pushed nothing at all.

## What went wrong

Their entries were therefore spent on somebody else's count.

- Measured, four reproductions across three subtools: `layer/add`, solo,
  `insert shape`, cage up, drag, `lattice/apply`, then **one undo — which
  deleted the subtool**. On another, one undo after `lattice/apply` removed the
  *stroke* and left the bend standing. On a third, one undo reverted a taper, a
  stroke, a conversion and an empty layer together. Inserting two boxes and
  undoing once removed four layers.
- The depth the interface reports never moved for any of them, four times over,
  so the panel said there was nothing new to take back while the document had
  changed.

One more, on the other side of the same arithmetic and found while fixing the
cancel in `#153`: **the commit path banked one entry per applied segment.**
That is the number a field gesture records; a mesh gesture is previewed while
it is made and banked as a *single* record however many segments drew it. So
one Cmd+Z after a three-segment mesh gesture walked the document's history back
by three — the gesture, and then whatever was under it, which on a fresh layer
is the layer.

## What this changes

- **Each of these banks exactly one action, where the change is made.** A
  ViewModel that writes to the document measures what the write cost and hands
  the count to the one that owns Cmd+Z, so a command added later cannot arrive
  without an entry. One shared accounting type carries the rule and the reason
  rather than four copies of it.
- **What a change cost is read from the history either side of it**, never
  assumed to be one entry: a subtool inserted with a shape in it is a layer and
  an item together, a boolean is two bakes and a layer, consolidating folds a
  whole list of nodes away. An operation the engine recorded nothing for banks
  nothing, which is what makes a refusal free without having to say so.
- **A manipulator drag is one action for the whole gesture**, press to release,
  measured from the depth the press read. Not one per frame, and not two when a
  frame overran and the document was left until the release.
- **A gesture's commit is measured from the depth it opened at**, exactly as
  its cancel already is, which retires the per-segment count on both sides of
  the gesture.
- **Ways of looking at the scene stay out**: which layer is active, what is
  drawn, which subtool is shown alone, and a hierarchy's stack of levels and
  passes, which stay adjustable long after the strokes that filled them.

## What this deliberately does not do

**It does not make these undos fast.** The 6.5 s and 51 s undos the audit
measured are refill cost, tracked with the UI-thread work.

**It does not reconcile the object table after a consolidation.** Folding a
layer's nodes away leaves placed objects listed that no longer have nodes; the
table is re-read after the command, and what the document answers with is the
document's business rather than the accounting's.

**It does not touch the import path.** Bringing a mesh in replaces the document
and drops the history with it, which is a different question from banking one.
