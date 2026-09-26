# Bind measure to the catalogue

## Why

The catalogue and the dispatch drifted in ways #146 did not cover (#191):

- `measure` built any route the dispatch held, including ones the catalogue
  does not offer, because the builder let an action with no row through.
- A retopology, UV layout, conform or bake runs on a worker thread, so a
  measured `run` stops the clock long before the work is done, and the answer
  did not say so. (`wait` reporting such jobs is #278.)
- `not_offered` named eight of the eleven commands withheld from agents; a
  bake's run and destination and the profile export were missing, so a caller
  could not tell "not offered" from "does not exist".
- Several summaries described something other than the action:
  `layer.set_combine` sets how the *next* SDF edit combines, `lattice.drag`
  moves exactly one selected point, `transform.drag`'s flag is a rotation snap
  offered as `invert`, flow is dab spacing, smoothing steadies the stroke's
  path, and both repairs are grid-only.

## What changes

- The action builder refuses a group or action with no catalogue row, so every
  group call and `measure` is bound by `GROUPS` and `TABLE`.
- A measured answer carries the work it left running and says the figure is
  the time to start it.
- `not_offered` lists every command `home_of` places as not offered, held by a
  test that reads `home_of`'s arms.
- The misleading summaries are corrected; `transform.drag` takes `snap`
  instead of `invert`. Each correction is checked by a test against the
  command or binding it describes.

## Impact

`transform.drag` with `invert` is now refused as an undeclared key; `snap`
replaces it. `Measured` gains `outstanding`, omitted when empty.
