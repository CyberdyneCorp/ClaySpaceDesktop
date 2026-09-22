# A boolean refuses what it cannot combine

## Why

The audit ran a boolean between two subtools and waited over a minute for
nothing (#181):

```
layer add {kind:'field'}   # base, sculpt something
layer add {kind:'field'}   # tool, sculpt something overlapping
boolean set {base:99, tool:100, op:'subtract'}
boolean run {}
-> 72,770 ms, no layer, no error, the panel still holding the pair
```

Two of the three things wrong with that have been fixed since. The minute was
the bake hiding and showing ~45 bystander layers around each operand and
refilling every one of them, which `a-refill-that-gives-the-frame-back` removed
by borrowing the visibility without refilling. The silence was the boolean's
notice channel not being among the ones the door read, which
`read-every-refusal-channel` fixed.

What was left is **why there was no layer**, and it was not the boolean's
fault. A brush stroke on a field subtool is a *relief* unless Combinar says
otherwise (`CombineSettings::for_strokes`), and relief offsets the field
already accumulated rather than adding one. On a freshly added subtool there is
nothing to offset, so the engine drops the item when it compiles the field.
The subtool nonetheless has an extent — every stroke item has a box — so the
panel offered it, priced it, and the run baked it: with only that subtool shown
the document compiles to nothing, and `clay_item_volume_from_document` refused
with "invalid argument (empty document)". That sentence now reaches the caller,
and says neither which subtool nor why.

Reproduced in `crates/clayspace-engine/tests/booleans.rs`: two field subtools
added and dabbed with the default brush over a scaffold sphere evaluate as far
(`CLAY_TAPE_FAR`) everywhere when shown alone, and the run fails in the sampler.

The fourth thing is that every one of these refusals arrived at *run*, after
the panel had accepted the pair, priced it and offered the button. A hierarchy,
the same subtool twice, an empty subtool — each is knowable when it is chosen.

## What changes

- **A subtool with strokes and no form of its own is refused by name**, with a
  refusal of its own (`BooleanRefusal::Formless`) rather than the sampler's
  words. Asked before sampling, with the operand already the only thing shown:
  one evaluation at the middle of the region, since an empty field is far
  everywhere. The subtool copy shares the bake and the refusal.
- **An operand is checked when it is chosen.** `ObjectModel::admit_boolean_operand`
  answers what a subtool *is* — gone, empty, or a subdivision hierarchy — and
  the boolean panel asks it on every `SetBoolean`, together with the same
  subtool being both base and tool. A refused choice writes the panel's notice
  (which the door reads) and leaves the pair the panel already held. The run
  asks the same question again through the same code, so the two cannot
  disagree.
- **A hierarchy is refused as a hierarchy** (`BooleanRefusal::Hierarchy`),
  before the other operand is baked, instead of as an engine string from the
  middle of the bake.
- Protection is deliberately *not* checked at choice: it is a flag the sculptor
  may lift between choosing and confirming, and the run checks it.

## What does not change

- Two field subtools that hold forms of their own — sculpted with Combinar set
  to Unir, or inserted as shapes — combine exactly as before; the new
  `a_boolean_produces_a_layer` holds that for the audit's shape of document.
- The operand contradiction between `make-representations-first-class` and
  `subtools` the issue mentions is already resolved in the living
  `scene-and-layers` spec (a mesh is not a *live* operand, and is a *resolved*
  one), so nothing is changed there.
- Whether a relief-only subtool *should* raise the surface beneath it — it does
  not today, on its own layer — is a sculpting question, not a boolean one, and
  is left alone here.

## Impact

- `clayspace-model`: two `BooleanRefusal` variants; `admit_boolean_operand` on
  `ObjectModel`, defaulting to admitting everything.
- `clayspace-engine`: the formless check in the operand bake; `operand_kind`
  shared by the choice and the run.
- `clayspace-vm`: `BooleanViewModel` validates `SetBoolean`.
- `clayspace-app`: `SharedDocument` forwards the new method.
