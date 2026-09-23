# A history that orders itself

The document keeps records the engine's history knows nothing about — a mesh
gesture, a crossing, the entries a solo left behind — and every one of them has
to interleave with the engine's own entries in the order the sculptor made
them. That ordering was a comparison against the engine's undo **depth**: each
record remembered the number it was made at, and an undo took the record back
when the number still matched.

A depth is a stack size. A stack size is not an ordering.

## What went wrong

**Two records at the same depth both answer "newest".** `undo_step` carried a
comment admitting it: a mesh gesture costs the engine no entry, so a solo
engaged before the stroke ended reported exactly the depth the gesture had
recorded, and which one an undo took back was decided by the order the
questions happened to be asked in. The workaround was to ask the mesh question
twice — once before the visibility hop and once after — which is not an
ordering either, only a tie broken in one direction.

**A depth reached again matches a record whose future has gone.** A mesh
gesture is not an engine entry, so the engine truncating its own redo stack
never reached the record. It sat on the document's redo stack remembering a
number, and the next undo that happened to bring the depth back to that number
handed it to a redo the sculptor meant for something else. Measured: a stroke
taken back, an unrelated subtool added, that subtool taken back, and the redo
meant for the subtool was spent putting the stroke on again instead.

**A depth that stops rising is worse.** The engine lowers it when a command
coalesces into the one before it, and stops raising it at all once a history
budget evicts as many entries as it gains. Every ordering decision built on the
number then answers "newest" for a record that is not newest, and undo walks
back the wrong entry — in the audit that meant layers removed by an undo that
was meant for a stroke.

## What this changes

The document counts for itself. One monotonic sequence, never decremented:
every engine entry is stamped as the document notices it, and every record the
document keeps takes a stamp from the same counter, in the order things
happened. An undo moves a stamp from one stack to the other rather than giving
the number back, so a stamp names one thing for the life of the process and `>`
on two stamps is a real answer.

- **One question, asked once.** "Which history holds the newest thing" is one
  function. `undo_step` no longer asks the mesh question around the hop, and
  the hop is what it says it is: a step over the entries nobody asked for.
- **A redo the engine has discarded is discarded here too.** A new engine entry
  ends the document's redo line exactly as it ends the engine's.
- **Eviction drops what it orphans.** A crossing or a visibility gesture *is*
  an engine entry; once that entry is gone the record goes with it, rather than
  waiting to be matched against an entry belonging to something else.
- **The object table follows entries, not depths.** It was filed under the
  depth it was recorded at, and any later edit reaching that depth was handed a
  table describing a different moment.

## What this deliberately does not do

**It does not raise the engine's history budget**, and it does not bind one.
The budget the audit blames is not reachable from the ABI this build binds:
`clay_document_undo_state` reports the depth, nothing sets or reads the budget
underneath it, and the engine's default is unbounded. The eviction rule here is
therefore stated and tested from inside the crate rather than provoked through
the public surface. That is deliberate — the ordering is wrong on its own
terms, and the two cases that *are* reachable today (a tie at one depth, and a
depth reached twice) are the ones the regression tests hold.

**It does not make undo fast.** The seconds-to-minutes undos the audit measured
are the refill cost, tracked separately.

**It does not change what a command costs.** A field dab that makes the engine
consolidate underneath it is still two entries, and the sculpting ViewModel
still banks a count per action. What changes is *which* entries a step moves,
not how many a command leaves.
