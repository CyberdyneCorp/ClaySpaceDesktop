# Tasks

## 1. The undo series

- [x] 1.1 Twenty mirrored Move gestures on one patch. Time the undo of each as the fastest of five, count the bricks it re-meshed, and print both with the chain length and the price of a brick.
- [x] 1.2 Assert the region: under a quarter of the surface, and no more than 1.5x the first undo's bricks.
- [x] 1.3 Assert the wall time against the 200x class: the last three under 8x the first three.
- [x] 1.4 Serialize the timing tests in the file so they do not slow each other down.

## 2. The pull

- [x] 2.1 A 60-sample Snake Hook pull, delivered the way the interface delivers it, with each segment timed; the late median stays under 6x the early one.

## 3. Documentation

- [x] 3.1 Re-record the v0.120.1 figures in `bound-the-chain-by-region` and `docs/why-move-was-slow.md`.
- [x] 3.2 Correct the tripwire's name to `a_baked_patch_has_no_decisive_undo_win` where the docs still use the old one.

## 4. Forty edits, and a pull on the grown layer

- [x] 4.1 Carry the series to forty gestures and report it at 1, 10, 20 and 40, asserting the region over all forty and the last three under 16x the first three. — `an_undo_after_forty_edits_stays_in_its_class`: 2.7–4.2x measured.
- [x] 4.2 Time a Snake Hook pull begun on the worked patch at the same checkpoints, first segment and median, and assert neither passes 16x its figure after one edit. — `a_stroke_on_a_grown_layer_begins_near_the_first`: 3.3–8.3x first segment, 1.7–4.5x median.
- [x] 4.3 Watch the price of a brick over a baked patch apart from the brick count. — `a_baked_patch_has_no_decisive_per_brick_win` in `tests/chain_compaction.rs`: 1.7–2.3x the chain's price on a loaded host.
