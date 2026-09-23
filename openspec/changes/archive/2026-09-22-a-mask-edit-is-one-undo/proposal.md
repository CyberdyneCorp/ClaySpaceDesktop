# A mask edit is one undo

`edit-history` has required since the beginning that every operation changing
document state be undoable, and `docs/features.md` says of the mask that "one
mask gesture is one undo". The engine holds up its half: a document-owned mask
writes a snapshot of its chunk map to the engine's history on every call that
reaches it, so the entries are there.

The history a sculptor presses is not the engine's. It is the sculpting
ViewModel's: **a stack of how many engine entries each action spent**, where
one Cmd+Z pops one count and undoes exactly that many. A stroke pushes its
count, an armature gesture banks its delta, a crossing and a grid repair were
given theirs in `#149` — and a mask operation pushed nothing at all.

## What went wrong

The mask's own entries were therefore spent on somebody else's count.

- The next undo popped the **previous** command's count and spent it on the
  mask's entries. The one after that reached further still, and the shortfall
  accumulated for as long as a session mixed mask edits with anything else.
- Measured through the agent door: `layer/remove 33; mask/apply clear; lasso
  outline; dab` and then two undos walked back **seven** entries — the lasso,
  the clear, the dab *and* the layer removal. Two subtools left the document by
  way of an undo meant for a mask edit.
- A mask edit made first in a session was not reachable by Cmd+Z at all, since
  there was no count on the stack to spend.

Three things in the same code came with it:

- **`steps` was accepted and ignored over the agent door.** The ViewModel
  overwrote the amount the command carried with the mask panel's own, and the
  menu had already filled that amount in before dispatching, so the overwrite
  served nothing and made `mask/apply {op:"expand", steps:4}` expand by one.
- **Clearing an empty mask cost a full edit.** The revision the viewport
  watches was bumped before the operation was even looked at and the clear was
  sent to the engine regardless, so a clear with nothing to clear re-uploaded
  1.78 MB of the layer and banked an entry.
- **`state.mask.present` answered the wrong question.** Inside the application
  it means "the layer carries a mask field", which stays true after a clear
  because a document has no verb for detaching one. On the wire it was read as
  "something is frozen", so an agent that cleared a mask was told the region
  was still protected.

## What this changes

- **One function applies every mask edit and banks what it cost.** All four
  entry points — the menu's operations, an outline, an extrusion and the
  clear — go through it, so an operation added later cannot quietly arrive
  without an entry. What the edit cost is read from the history either side
  rather than assumed to be one, because an outline enclosing two pieces of the
  form is a group and an extrusion adds a layer beside the item it made.
- **The amount applied is the command's own**, brought inside what the engine
  takes and said when it had to be brought in.
- **Clearing an empty mask writes nothing**: no snapshot, no entry, no
  re-upload, and a remark rather than a refusal — the mask ends up exactly as
  the caller asked for it.
- **The wire says whether anything is frozen.**
- **A history step tells the viewport to look at the mask again.** Every site
  that writes to a mask bumps the revision beside the write; the engine's
  history writes through a snapshot, past all of them, so an undo restored a
  region nothing would redraw.

## What this deliberately does not do

**It does not make the mask's undo fast.** The seconds-long undos the audit
measured are the refill cost, tracked separately.

**It does not change what a mask operation costs the engine.** The snapshot per
call is the engine's design; what changes is that the entries are accounted for
on the side that spends them.
