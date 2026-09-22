# Guards in the model

## Why

Two rules that protect an open gesture and a raised cage were written where the
pointer is handled. Anything that reached the document another way — the agent
door in particular — walked straight past them. Measured over the door:

| guard | what got through |
|---|---|
| a gesture in progress | `history/undo` and `layer/add` applied in the middle of the agent's own open stroke |
| a structural operation during one | `layer/remesh` ran with a mesh gesture open, leaving `clay_mesh_sculptor_flush_normals` and `clay_mesh_deltas_revert` errors and a band of clay that survived the undo |
| a cage that is up | brush strokes were accepted while a lattice cage stood — the very thing the pointer path refuses |
| the cage's own target | a cage drag moved and turned a hidden object at the same time, so one drag changed two things |

The gesture guard had a deliberate exemption: an agent must be able to finish a
stroke it started, so a gesture the agent itself opened did not count as a
gesture. Written as "this is not a *person's* gesture" rather than as "this
belongs to the gesture that is open", it exempted **every** command while the
agent held one.

The cage rule lived in `input::press_sculpts`, which only the pointer handler
consults. No equivalent check existed in the sculpting ViewModel, so
`stroke/begin` over the door was unaffected by a raised cage.

## What changes

- **The exemption becomes narrow.** While a caller holds a gesture of its own,
  only the verbs that carry it on or close it go through. Everything else that
  would change the document is refused, naming the open gesture.
- **The command vocabulary owns the three gesture sets.** Which verbs open a
  gesture, carry one on and close one are asked of `Command` rather than listed
  at each caller. The composition root had one copy of that list and the door
  had another, and the two were not the same list.
- **"Changes the document" is asked the wide way.** `touches_document` answers
  a narrower question — is this an entry in the undo history — and seven
  commands that plainly change the document answer `false` to it because they
  mark the document on the composition root's own path. A crossing, an import
  and a pass of the active layer's stack are among them, and they are exactly
  what must not run inside a half-finished gesture.
- **A cage refuses strokes in the ViewModel**, where every caller passes,
  rather than in the pointer handler alone. The pointer's own check stays as a
  second line.
- **Raising a cage clears the whole-subtool manipulator's target**, which the
  code already assumed and nothing did.

## What this deliberately does not do

**It does not move the refusals into the document.** The engine has no notion
of "a gesture the interface is holding" — the ViewModel layer is where an open
gesture is known, and pushing the rule below it would mean teaching the engine
a concept it does not have.

**It does not make a cage grey out the brush shelf.** A cage refuses a stroke;
the shelf still offers the tools, because the cage is a mode a sculptor is
about to leave rather than a property of the layer. Folding it into
`LayerState` would change what every availability question answers.
