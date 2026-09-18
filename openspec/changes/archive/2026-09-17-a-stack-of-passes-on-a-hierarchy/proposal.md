# A stack of passes on a hierarchy, dialable long afterwards

## Why

`a-hierarchy-the-domain-can-describe` put the pass stack in the domain and
said, in as many words, that nothing writes into one. `Hierarchy::state`
returned an empty stack with the reason written beside it: nothing in this
application wrote a pass, so a row read back from the engine would always be a
pass the engine made and no sculptor asked for.

Everything needed to change that was already there and unreached.
`SceneModel::apply_multires_sculpt_layer_op` had a default refusal and no
implementation, so eleven operations the domain spells out reached nothing.
`crates/claycore` wraps all thirty-nine of the stack's entry points and nothing
above the wrapper called one. And `MultiresSculptLayer` — an id, a strength, a
visibility, a lock, a mask, a coverage — described rows that could not exist.

The claim a pass makes is not "a stroke went somewhere else". It is **that a
stroke stays adjustable after the pointer comes up**. A strength that only
worked during the gesture that made the pass would be a stroke modifier wearing
a layer's clothes, and it is the one thing about this feature that an interface
can quietly fail to deliver.

## What Changes

- **The stack is read back from the engine and acted on through it.** Every row
  comes from `clay_multires_sculpt_layer_*` each time it is asked for: a merge
  and a bake rewrite the stack wholesale and a restore from bytes replaces
  every id there was, so a host-side copy would be one undo away from
  describing a stack that is not there. A reorder is the engine's move rather
  than a `Vec` rotation, and a merge is the engine's fold rather than the
  obvious arithmetic — which divides by the lower pass's strength, and zero is
  a state one slider reaches.
- **A stroke enters the active pass**, through the layered stroke transaction,
  which is a different entry point and not a flag. The transaction fixes the
  channel at pointer-down, so changing the active pass mid-drag cannot split
  one gesture across two, and holds the composition for the length of the
  gesture, so a stamp is not summing the stack again between dabs.
- **The stack is drawn under the layer it stands on**, in the same shape a
  grid's passes take, because a sculptor working a second representation should
  not meet a second layer idiom.
- **The form under the passes has a row of its own**, at the bottom of the
  stack, and selecting it sends the next stroke back into the surface itself.
  That row is the whole of the write domain as this application expresses it.
- **A pass is reordered by dragging it**, not by two arrows. On a grid the top
  of the stack wins where two passes overlap, so arrows say something about the
  result; here the stack is a sum and a reorder is defined to move nothing, so
  the gesture is the one for organising a list.
- **A save that could not write a hierarchy's sculpt says so.** The document
  ViewModel's notice has said "the last failure, for the interface to show"
  since it was written and nothing showed it, so a save that failed on the
  side-car went to stderr.

## The write domain is a row and not a control

The engine takes three write domains — automatic, the form, the active pass —
and this application sends only `Automatic`. That is a decision rather than an
omission. `Automatic` means "the active pass, or the form where there is none",
so **which row is selected is the whole answer** to where the next stroke goes.
A three-way control beside the rows would be a second way to say the same
thing, and the two would disagree the first time one of them moved.

## What a pass stroke does not carry

The layered transaction offers stamps and no stroke resolver — clay.h carries
`clay_multires_sculpt_layer_stroke_stamp` and no `_apply_stroke` beside it — so
the preset's jitter and taper do not reach a pass. The samples that arrive are
already about one dab's travel apart, because the sculpting ViewModel spaced
them before sending, so the coverage is right; a pass stroke is that much more
even than the same stroke into the form. Stated here rather than left to be
noticed.

## Out of scope, and why

- **Authoring a pass's mask.** The engine stores a per-vertex weight with each
  pass, whose identity is 1, and offers no call saying whether one is stored —
  only a reader per vertex. This application cannot write one either: the
  freeze a sculptor paints is a volume rather than a per-vertex weight. The row
  carries the badge, and the badge cannot light. A mask on a pass wants the
  freeze to be resolvable per vertex first, which is its own change.
- **A pass in the undo history.** Dialling a pass is a property of the stack
  that stays adjustable long after the strokes that filled it, so it is not an
  entry — the same answer a grid's passes already get. A sculptor whose next
  undo took back a slider rather than the work would have to choose between the
  two.
- **A benchmark figure.** A pass stroke is a new figure key, which exists only
  on the B side of the standing A/B and cannot be compared to anything.
