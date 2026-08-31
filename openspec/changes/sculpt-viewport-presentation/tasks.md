# Tasks

## 1. The grid

- [x] 1.1 Establish where the fade can live: overlay geometry is uploaded when
      the overlays change and not per frame, so a camera-dependent colour would
      rebuild it on every orbit
- [x] 1.2 Fade by distance from the origin, mixing toward the viewport ground,
      with a smoothstep so the grid thins rather than ending on a ring
- [x] 1.3 Cut each line into segments. A line drawn as two vertices takes the
      interpolation between its ends, and both ends of a grid line are equally
      far from the middle — so the first version faded every line uniformly and
      dissolved nothing
- [x] 1.4 A major line every fifth, with the axes strongest
- [x] 1.5 Tests: the outermost vertex has reached the ground; a single line
      fades along its length; a major line outweighs the minor one beside it,
      compared at a radius where neither is faded

## 2. The quality profiles

- [x] 2.1 Offer the three from the View menu, beside the shading terms
- [x] 2.2 Leave the choice in the interface's memory and read it after the
      frame — it is a view type, so it cannot be a command
- [x] 2.3 Names in three locales, and into `all()`
- [x] 2.4 Test that choosing one is left where the composition root reads it,
      and emits nothing
- [x] 2.5 Move `LANGUAGE_ENTRY`, which is a pixel offset down the same menu and
      whose own comment predicted this

## 3. Verification

- [x] 3.1 Look at the grid
- [x] 3.2 `just check` — fmt, layering, clippy, spec, packaging and the
      workspace suite all pass
