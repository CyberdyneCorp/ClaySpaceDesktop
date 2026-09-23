# State reports enough to verify a command, and names the next history step

## Why

`state` was assembled from whatever the ViewModels happened to publish as each
feature landed, so its coverage follows the order the application was built
rather than what a caller needs in order to check its own work. Twice during
the audit, state the report does not expose corrupted a test without the tester
noticing, and most of the audit's verification was pixel comparison because
`state` could not answer.

Measured:

- **`undoes` named the last thing that happened rather than the next step.**
  After an undo it reads "undo"; after a cancelled clay stroke it reads
  "Argila", which is the tool whose stroke the cancel had already taken back.
  There was no redo depth at all, so an agent that undid four things could not
  tell how many redoes would put them back.
- **`scene.layers[].objects` was the length of `sculpt_layers`** — a *grid's
  recorded passes*. A field layer holding a dozen placed shapes reported none,
  and a grid reported its passes under a name that says objects.
- **Nothing said a scene was soloed.** From the report, a solo and a sculptor
  hiding two layers by hand look identical.
- **`selected_object` went stale.** The document answers with the id it was
  last told about, so an undo that removed the form left the selection naming a
  node no longer in the list.
- **`tool.symmetry` ignored the rig's mirror**, which is a different switch
  from the brush's and decides whether a new ZSphere gets a partner.
- **The two memory figures were reported as one.** The status area shows the
  brick cache (0.00 GB); `state.memory` shows the document's ledger (359 MB).
  Neither is wrong and nothing said which was which.
- **Nothing reported at all:** the brush panel's settings, the combine mode,
  the mask's steps and gesture, the cage, the deform parameters, the placed
  forms with their node ids, the grid passes, the hierarchy's levels and
  passes, the last rebuild, retopology or crossing, how the viewport is
  presented, the reference images, or the exchange settings.

The history half of this depends on the sequence `a-history-that-orders-itself`
gave the document; the rest was independent and had simply never been written.

## What changes

- **The next step, in both directions.** The sculpting ViewModel banks a
  *label* beside each action's entry count, and the label travels with the
  count as an undo moves it to the redo stack and back. `history.undoes` names
  what the next undo would revert, `history.redoes` what the next redo would
  restore, and `history.redo_depth` joins `depth`. `Applied.undoes` answers the
  same question, so a command that banked nothing names the command before it.
- **Thirteen sections the report did not have:** `brush`, `combine`, `cage`,
  `deform`, `objects`, `outcomes`, `presentation`, `references`, `exchange`,
  plus the grid passes, the grid's own cells, the hierarchy's levels and passes
  and the solo flag nested where they belong.
- **`objects` means one thing.** A count of the forms placed in the layer, on
  every representation. The passes are reported under `passes`, named.
- **`selected_object` is cleared when its node is gone**, against the list the
  document actually holds, exactly as the manipulator's target already was.
- **The rig's mirror travels beside the brush's**, as `tool.rig_mirror`, and
  only while a rig is being edited — for the reason the smooth frequency is
  sent only on a hierarchy: a switch reported where it decides nothing is a
  switch an agent will act on.
- **Both memory figures are named.** `memory.cache_bytes` is the status area's
  own, beside the ledger's `in_use_bytes`, so an agent and a person can see
  which figure each is reading.
- **The section list is written once.** `StateQuery::NAMES` drives the reader,
  the refusal that names the sections that exist, and the tool schema an agent
  reads. A section in one and not the others is no longer a thing that can be
  written.

## What this deliberately does not do

**It does not reconcile the two memory figures into one.** They count different
things — the brick cache the budget bounds, and the document's ledger with its
surfaces — and neither accounts for host-owned memory. Making one figure out of
them is the memory-accounting work, and is a change of its own. This makes both
visible and says which is which, which is what stops them looking like a defect
in one of them.

**It does not add a table test over the whole command list.** The issue asked
for one in `crates/clayspace-app/tests`, walking every command and asserting
`state` changed; that needs a window and a built engine, and half the commands
need a document in a particular shape first. What holds the coverage instead is
mechanical and runs with no engine: the section list, the query's fields and
the keys the report writes are asserted to be one set rather than three that
happen to agree today.
