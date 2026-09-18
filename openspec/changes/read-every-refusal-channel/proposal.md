# Read every refusal channel

## Why

"Refusals reach the caller" gave the composition root's own operations the
channel they never had, and said in as many words what it was leaving behind:

> It does not find readers for the notices the other ViewModels write. The
> lattice, the curve and the cut each write a refusal to an Observable nothing
> reads; the armature's goes to `eprintln!`; the boolean's is drawn in its own
> panel and is not one of the channels the door counts.

That is this change. The door decided whether a command was refused by
comparing five channels either side of it, and the application has fifteen. Ten
ViewModels wrote their refusal onto a channel nobody read, so the refusal was
recorded correctly and then discarded. Measured:

```
boolean set {base:<layer A>, tool:<a hierarchy layer>}
boolean run {}
-> {"label":"run boolean","touched_document":true}   # isError: false
   no layer produced, 73 s spent, nothing said on any surface
```

```
cage up {}
lattice drag {points:[], delta:[0.1,0,0]}
-> success, with the drag refused and the sentence written where nobody reads
```

A scale the brick cache refused came back the same way, with the readout still
showing 10× and `view/frame_all` framing bounds that do not exist. A refused
cage apply threw away every drag the sculptor had made without a word — the
cage had no line in the options bar either, so the refusal reached neither
reader.

The cause is that the list of channels was a list, written out by hand twice
inside two methods that had to agree position by position, and nothing held it
against the ViewModels it was supposed to cover. Each missing channel has been
found the same way: by somebody driving the application and noticing that
nothing happened.

## What changes

- **Every ViewModel that can refuse is read at the door.** The cage, the curve,
  the boolean, the rig, the cut, the reference panel and the four panels that
  run their work off the interface thread join the five channels already
  compared. A refused boolean run, a refused cage command, a refused curve edit
  and a refused ZSphere verb are errors carrying the reason, not successes.
- **A refused cage, curve or rig says why on screen.** The cage had no line of
  its own at all. The three join the options bar's one "why that did not
  happen" line, after a refused transform and ahead of a standing condition.
- **The rig's refusal stops going to stderr.** It was printed from the
  composition root and written nowhere else, which is the same dead end the
  other refusals were in before the last change.
- **The list is written once.** `App::refusal_channels` and
  `App::remark_channels` are the registry; the count before a command and the
  words after are both read from it, so a channel added once is read twice and
  a channel in one list and not the other is no longer a thing that can be
  written. A channel added without widening the count is a compile error.
- **Forgetting to register a panel fails a test.** `every_notice_channel_is_read`
  reads the composition root's own `struct App` for the ViewModels it holds,
  reads each of those ViewModels for the notice channels it owns, and fails
  naming any the registry does not read.

## What this deliberately does not do

**It does not carry a background job's refusal.** A retopology, a UV layout, a
conform or a bake that fails *while running* writes its notice when the job
completes, which is between commands rather than during one — so the door
cannot attribute it to anything, and an agent that started the job is told it
started. Their refusals *at the start* — nothing to bake, no maps chosen, a job
already running — are covered here, because those are answered on the spot.
Carrying a completion back to whoever asked for it needs the job runner to
report it, and that is a change of its own.

**It does not change what any ViewModel refuses.** Every sentence in this
change was already being written; the only difference is that somebody now
reads it.

**It does not fix the readout that keeps a refused value.** A scale the engine
refused now says so, on screen and at the door. That the transform panel goes
on displaying the number it was typed is a separate defect in what the panel
reads back, and is tracked with the transform work.
