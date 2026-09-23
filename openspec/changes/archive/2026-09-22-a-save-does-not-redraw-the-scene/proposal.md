# A save does not redraw the scene

## Why

Autosaving a document with a subtool soloed froze the window for the length of
a full visibility round-trip, and then did it again. Measured as one continuous
episode, seen three times:

- the interface thread sat in `SharedDocument::save → write_visibility →
  drain_dirty` for about **146 s**;
- the next autosave began immediately — the watch log shows them back to back
  with no idle gap;
- the application was unusable throughout.

Two things combined.

**The save moved the live scene to write a file.** A solo is a way of looking at
the document, so the file has to record what the sculptor set rather than what
they were looking at — otherwise a reopened document, and the crash recovery
nobody gets to check before trusting it, comes back with everything but one
subtool hidden. `save` did that by writing the sculptor's pattern into the live
document, saving, and writing the solo back. Both of those go through
`write_visibility`, which marks every field subtool whose eye moved and drains
the brick cache before returning. So one save refilled the whole scene twice,
on the interface thread, for a file.

That refill is not going away: hiding a worked field subtool genuinely changes
the field, as "An eye costs what it moves" says in as many words. What is wrong
here is that a *save* was paying it at all.

**The clock ran during the save.** `maybe_autosave` stamped the clock before
the attempt, so the interval was measured from one save's start to the next
one's start. A save longer than the interval is therefore due again the instant
it finishes, and the application spends its life saving. The stamp was early on
purpose — a save that keeps failing must not be retried on every turn of the
event loop — and that concern is real; it is the placement that was wrong.

## What changes

- **The flags go into the file and only into the file.** The save lends the
  engine the visibility the sculptor set for the length of the write and takes
  it straight back, through the `with_borrowed_visibility` bracket a bake
  already uses: nothing is marked for refill and nothing is drained, so the
  cache goes on holding the fill the solo describes — which is still what the
  viewport is asked to draw when the save returns. What the file contains does
  not change at all.

  One bracket and not two. `a-refill-that-gives-the-frame-back` reached the
  same conclusion about a save from the other side, and the mechanism is
  shared rather than duplicated: a save lends its pattern for exactly the
  reason a bake does.
- **The borrowed flags cost no undo step.** The engine records a command per
  flag whatever the host wants, so the writes are filed as visibility gestures
  and undo hops the run of them, exactly as it hops a solo's own writes. A ⌘Z
  after a save reaches the sculptor's edit.
- **The autosave clock is stamped after the write**, so the interval is idle
  time between saves rather than time from one save to the next. It is stamped
  whether the write succeeded or failed, which keeps what the early stamp was
  protecting: a failing save waits out a full interval like any other.
- **An autosave is skipped while a gesture is open.** A save reads the whole
  document, and a stroke, a drag or an outline is a hand still moving. The tick
  is not lost — nothing about skipping it restarts the clock, so it is taken as
  soon as the gesture ends. The event loop schedules no wake-up for it
  meanwhile, or a deadline already in the past would spin the loop for as long
  as a sculptor held the pointer still.

## What this deliberately does not do

**It does not make a manual save cheaper on an unsoloed document.** There was
never a visibility round-trip there, and the write itself is the engine's.

**It does not move the save off the interface thread.** That is a larger change
than this one, and with the round-trip gone what is left is the write, which is
the work the sculptor asked for.
