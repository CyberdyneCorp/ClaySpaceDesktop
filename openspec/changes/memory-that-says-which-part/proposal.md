# The memory report says which part, and counts the surfaces it asked

## Why

The diagnostics report — the text a person pastes into an issue — carries no
memory figure at all. The one memory figure the application does show, in the
status bar, is `clay_brick_cache_stats.memory_usage` against its budget: one
brick cache, presented as if it were the document. It leaves out the edit list,
the voxel content, the mesh layers, the masks and the whole undo history.

Neither answers the question a memory warning actually raises. A sculptor who
has just been told memory is short does not need to know how big the document
is. They need to know **which part**, because that is what decides what they
are allowed to let go of.

ClayCore 0.78.0's `clay_memory_report` carries the three roll-ups that answer
it — `essential`, `rebuildable`, `undoable` — derived by the engine from its
own category lines rather than counted beside them, so a line added upstream
without being classified cannot make them disagree with the total.

And there is a second, sharper reason. A `clay_mesh_sculptor` is an owning
handle this application holds **beside** its document rather than inside it, so
`clay_document_memory` reports the whole surface tier as zero. That is correct
— the engine cannot walk what it does not own — and it means a host that stops
there publishes a figure that omits the largest thing on the machine. Measured
on the starting form crossed to a mesh and dabbed once: the plain roll-up says
8.46 MB and the session beside it is another 8.45 MB. The release notes are
blunt that at twenty million vertices the plain roll-up "gets a number that
omits the largest thing the artist is holding". This application should not be
that host.

## What changes

- `Diagnostics` gains a `memory` section carrying the three roll-ups, the
  total, how many surfaces were asked for a ledger, and what they came to. It
  reaches the pasted report and a **Memória** section in the diagnostics
  window.
- `ClayDocument::surface_ledger` asks every mesh-sculpting session the document
  holds for its `clay_memory_ledger` and merges them. The merge is this side's
  job because only this side knows which sessions belong to which document,
  which is why the engine merges none.
- `ClayDocument::memory` hands that ledger to
  `clay_document_memory_with_surfaces` rather than calling the plain roll-up.
- The surfaces row is reported at zero as well as at two. A surface tier of
  zero is the right answer on a document holding no session, and it is also
  exactly what a host that never filled the ledger would print — the count is
  what tells the two apart.

## What this does not do

- **The status bar keeps its brick-cache figure.** Changing what that meter
  measures is a change to a control a sculptor watches while working, with its
  own threshold and its own wording, and it wants to be proposed as one.
- **Nothing is released.** `clay_memory_pin_*` and the trim entry points are
  wrapped and untouched here: this change makes the figure honest, and acting
  on it — releasing `rebuildable` under pressure, shortening the history — is a
  policy, which is a decision rather than a readout.
- **`NoticeBoard` is still not constructed.** Its `set_memory` and
  `budget_exceeded` are the right shape for a warning with a remedy, and the
  roll-ups are what would make its generic remedy specific. That is the change
  that should also wire it into the composition root.

## The one implementation detail worth writing down

The ledger is accumulated onto the *first* surface's answer rather than onto a
default one. `MemoryLedger::merge` carries the shorter of the two category
counts, so folding into a zeroed ledger would report every category as unfilled
however many surfaces were added to it.
