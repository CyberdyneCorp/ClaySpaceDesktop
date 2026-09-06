# A pull that does not slow down as it is drawn

## Why

A sculptor reported the Snake Hook stalling, with the stall growing as the pull
went on: `continue stroke` at 89 ms, then 226, then 335, then 440, ending at
`stroke 558 ms`. Cost rising with stroke length is the signature of quadratic
work, and it was.

**Measured before anything was blamed.** A probe drove the three engine calls a
segment makes, at increasing curve lengths:

```
points   set_pts ms    mark ms   refill ms   bricks
     4        0.051      0.010      36.702      252
    20        0.068      0.045      59.175      448
    40        0.166      0.045     110.432      880
```

The engine's own calls are not the cost — `clay_layer_set_stroke_points` is
0.05–0.17 ms and `clay_brick_cache_mark_dirty_nodes` is 0.01–0.05 ms across the
whole range. **99.8% is the refill, and the refill was honest work**: the brick
count scales with the tendril, and the whole tendril really was dirty.

It was dirty because of the taper. Each control point's radius was
`size * (1.0 - 0.7 * t)` with `t = index / (len - 1)`, and that denominator is
however long the stroke has turned out to be *so far* — so every extra sample
renumbered every point and rewrote its radius. A point already laid down at
index 5 thickened by **82%** over one forty-sample pull. Two defects in one:
clay behind the cursor kept fattening as the sculptor drew, and because the
field genuinely changed along the entire curve, the whole node's bound was
correctly dirty and correctly re-evaluated, every segment.

So `clay_brick_cache_mark_dirty_nodes` computing the node's whole bound was the
**correct answer to the question being asked**. The question was wrong. That is
the hardest kind of performance defect to find, and it was found only by
measuring where the time went before deciding whose it was.

## What Changes

- The taper is measured along the **path** rather than across the point index,
  so a control point's radius is fixed the moment it is placed.
- A segment dirties the region its newest samples changed, and each image the
  layer mirror puts that region at, instead of the whole node.
- `dirty_bricks` reports what a segment actually dirtied, where the grow path
  returned the constant `1`.

The first two are one fix. While a radius could still change behind the cursor
the whole node genuinely was dirty, and narrowing the region would have been
wrong rather than fast.

## Impact

**Per segment: 150 bricks against 880.** End to end, one `continue stroke` at
the segment size the application really sends, walked out to a full pull:

```
            no mirror              mirrored in x
samples    ms    bricks         ms     bricks
      6   0.554     80        0.953      96
     39   0.854     48        3.767      96
```

Unmirrored is flat. The reported 440 ms segment is now under 1 ms, and under
4 ms at the worst mirrored.

**One conclusion in this work was wrong and is corrected rather than removed.**
The taper span is five because a fatter tendril renders with specks of
background through it — and the first reading of those specks, that they were
holes in the engine's mesher because they appeared in its own mesh as well as
in ours, does not follow: all three pictures go through one rasteriser.
Measured topologically the document is watertight, 2-manifold and Euler
characteristic 2 at three resolutions. It carries sub-pixel slivers instead,
which a rasteriser drops. The span still stands, because a sculptor sees the
specks; what changed is whose defect it is, and the answer is ours.

**What remains is not ours.** The mirrored column climbs while its brick count
does not, which is `ctape_stroke_dist` walking every segment of the curve for
every sample — O(control points), confirmed by the engine's authors in
`include/clay/kernel/tape.h:488`, twice over for a mirrored stroke because each
image is its own instance. A per-segment bound would early-out under the smooth
blend. It is filed and deliberately not scheduled, and the workaround it would
make pointless — chaining several short items instead of growing one long one —
is **not taken here**: it trades a few milliseconds against the brush's whole
appearance, and the appearance is what the single grown curve exists to protect.
