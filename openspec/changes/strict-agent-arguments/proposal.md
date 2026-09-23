# Arguments the door cannot honour are refused, and a clamp is reported

## Why

The agent door's argument decoder accepted values it could not honour and keys
it did not know, and answered success over both. The caller was told the call
applied and was left with a document configured differently from what it asked
for (#187):

```
brush set_size {sizee:0.5}            # a misspelling: applied, at the old size
layer set_remesh {resolution:-1}      # -1 as u32, clamped: 512
exchange set_import {scale:1e308}     # an infinity once it is an f32: "Scale inf"
exchange set_import {max_vertices:-1} # u64::MAX
reference set {offset:"bad"}          # the refusal thrown away: the default offset
```

Each is the same failure: the decoder cast where it should have checked, or
looked up the keys it knew and never asked about the rest. And where the
application brought a value into range — an opacity of 5 drawn at 1 — nothing
in the answer said so, so a caller had no way to find out except by reading the
state back and noticing.

Reading the decoder for this turned up one more: `brush set_azimuth` is
documented, and drawn on the panel, in degrees, and the panel converts to
radians before pushing the command. The door passed the degrees through, so 45
was applied as 45 radians.

## What changes

- **Unknown keys are refused.** A key the action's table row does not declare
  is refused with `bad_argument`, naming it and listing the keys the action
  does take. The call's own envelope — `action`, and a `capture` with its
  `width` and `height` — is not an argument of the action and is let through.
- **A whole number that does not fit is refused, never cast.** Counts, sizes,
  keys and indices are read through one checked conversion; a negative count or
  one too large for its field is refused with the value given. A selection is
  cleared by leaving its index out, as `describe` says, and no longer also by
  any negative number.
- **A number that is not finite as an `f32` is refused**, in every numeric
  argument and every list of numbers. JSON cannot carry a NaN or an infinity,
  but `1e39` becomes one on narrowing, and that is the value checked.
- **A clamp is reported.** Where the application brings a value into range
  rather than refusing it — a brush's numbers, a rebuild's resolution, a
  surface's opacity, a grid's blur, and the fields each settings block's
  `sanitized` moves — the answer carries `clamped: [{argument, asked, used}]`.
  The rule applied is the model's own `sanitized` or constructor, asked rather
  than restated, so the report cannot drift from what is applied.
- A malformed reference offset, an unknown retopology method and an unknown
  bake map are refused instead of falling back to a default.
- `brush set_azimuth` converts its degrees to radians, as the panel does.

## Out of scope

- `pick_recent_colour` past the end of the recent list, and `set_alpha` on a
  field: what is valid there depends on the session rather than on the
  argument, so the refusal belongs to the ViewModel that knows it.
- `mask apply {steps}` was reported ignoring its steps; the catalogue carries
  them into the command, and the mask ViewModel honours them on main. A test
  here now holds the catalogue's half.
- The retopology, UV, conform and bake verbs have no table row (no group offers
  them; only `measure` reaches them), so they have no declared list to refuse
  unknown keys against. Their values are still range-checked.
- The five tools that are not commands (`describe`, `state`, `viewport`,
  `wait`, `measure`) keep their own argument handling.
