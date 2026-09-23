# A held smoothing gesture keeps every dab when the pointer comes up

## Why

A sculptor dabbing a spot smooth with Suavizar on a field, without letting go,
watched the result compound in the preview and then spring back to what the
first dab had done the moment the pointer came up.

The preview relaxes the transaction's retained volume once per dab. The release
threw the preview away and replayed the gesture through the held-stroke path,
`relax_stroke`, which makes ONE relax pass about the centre of the stroke's
bounding box at the brush radius. Dabs stacked in one spot collapsed into one;
a long or curved stroke kept only a brush-sized spot at its middle, which can
lie off the surface altogether. The requirement that the surface come to rest
where the preview showed it was measured on a gesture short enough that the two
agreed, and so never caught it. Relaxar shares the path and had the same fault.

## What changes

- The live gesture records one dab per segment — its brush and centre — where
  it used to keep the raw samples.
- Releasing it samples the region the dabs reached once, relaxes those samples
  by each dab in turn with the parameters the preview used, and places the
  result as one Replace item, per mirror image. The preview and the release
  share one function for a dab's relax parameters so the two cannot drift.
- The transaction's own commit is still not used: it consolidates the whole
  layer, which the requirement forbids.
- The held (non-live) path is unchanged.

## Impact

- `crates/clayspace-engine/src/document.rs`: `close_live_gesture`,
  `relax_dabs`, `live_relax_params`, `LiveDabs`.
- `crates/clayspace-engine/tests/live_smooth.rs`: the regression test
  `every_dab_of_a_held_gesture_survives_the_release`.
- `sculpting-tools`: the live-gesture requirement says the result carries every
  dab, and gains the scenario that pins it.
