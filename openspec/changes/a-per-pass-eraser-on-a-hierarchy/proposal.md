# A per-pass eraser on a hierarchy

A hierarchy has an eraser nobody was ever offered.
`clay_multires_sculpt_layer_stroke_erase` takes the selected pass's detail
channel toward zero and touches neither the base nor any other pass; the
wrapper has bound it since the multires module was written
(`SculptLayerStroke::erase`). No stroke in this application opened it, because
`ToolKind::Apagar` was `multires: None` and the row even said so — "not to be
confused with the hierarchy's two erasers, which are gestures inside a layered
stroke rather than verbs of their own here yet".

The capability matrix recorded Multires Erase as an absence that was
structurally appropriate. The engine's own wording disagrees: the verb exists
precisely because erasing *one pass's* detail is a multiresolution operation
with no mesh equivalent, and the engine comment calls it "an eraser for THIS
pass rather than a flattening brush". Without it, an artist who wants a pass's
deposit gone has to flatten or smooth, and both of those reach the form
underneath — which is the one thing the pass stack exists to keep separate.

## What this changes

- **Apagar is bound on a hierarchy**, to the layered stroke's erase, and joins
  the shelf for that representation.
- **It is refused, with a sentence, where the form is selected rather than a
  pass.** Not redirected: the engine's erase walks *the target channel* toward
  zero, and with the form selected that channel is the base detail — so the
  same gesture would take the whole surface back toward the pure subdivision.
  That operation exists under another name (`restore`), it is destructive at a
  scale an eraser does not suggest, and nothing on the shelf would have told a
  sculptor which of the two they were about to get.
- **The tool carries a caveat on a hierarchy**, because this is the one label
  in the table over two genuinely different operations: on a grid Apagar clears
  the cells the brush covers, and on a hierarchy there are no cells to clear.
- **`LayerState` gains the row a stroke would land in**, so that the refusal is
  a property of the state a tool is asked about rather than a rule written into
  one call site. Every representation but the hierarchy answers "the form".

## What this deliberately does not do

**No engine work and no new FFI.** The entry point, the wrapper and the
transaction have all been there since the pass stack landed; the change is the
application reaching them.

**It does not bind `restore`.** Taking a level's own detail back toward the
pure subdivision is a real operation and a different one, with a different
refusal (it is refused at level 0) and a different place in the interface. It
is not an eraser and should not arrive wearing an eraser's label.

**It does not hide the tool where the form is selected.** A brush that vanished
when a sculptor clicked the form's row would leave nobody to say why, which is
the failure the shelf's own rule about browsed tools already names.

**It does not make a pass stroke resolved.** The transaction offers stamps and
no resolver, so the erase is stamped sample by sample for the same reason every
other pass stroke is.

## Capabilities

### Modified Capabilities
- `sculpting-tools`: the hierarchy's vocabulary is no longer exactly the mesh's
  less its colour — it gains the one verb a stack of passes makes meaningful —
  and the eraser there states what it does and refuses the form.

## Impact

**Code**: `clayspace-model` (`Apagar`'s multires binding, the pass row on
`LayerState`, the `NeedsAPass` refusal, the tool note), `clayspace-engine`
(`document.rs`: the erase routed through `SculptLayerStroke::erase`, and the
document's answer for which row takes the stroke), `clayspace-app`
(`shared.rs`: forwarding that answer), `clayspace-view` (the caveat in three
locales).

**Risk**: low and bounded to one tool on one representation. The regression
that matters is measured rather than argued: the erased pass is hidden and what
is left — the base and the other pass — is compared vertex for vertex against
what it was before the stroke, which is an equality rather than a tolerance
because a hidden pass contributes exactly zero.
