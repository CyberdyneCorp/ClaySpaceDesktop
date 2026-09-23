## Why

Two commands could multiply a document's size by an order of magnitude and neither had a price attached.

A shape parameter was capped at a fixed `10.0` — `crates/clayspace-model/src/shape.rs` — a number chosen for "room to place something large beside the reference form" and measured against nothing. A sphere at radius 4 sat comfortably inside it and added **4.51M triangles in one insert**; the same path produced a 16M-vertex surface after which the session never recovered, at a 26 GB footprint.

A curve radius had no upper bound whatsoever. `set_curve_radius` in `crates/clayspace-engine/src/document.rs` clamped to a minimum of 1e-3 and to nothing above it, so `curve/set_radius 5` was accepted: the application held for more than thirty seconds and reached **4.58 GB**. The brick cache then reported a dirty region of 2.3M bricks against a limit of 6.7M, and a region past that limit left a curve that could not be removed — a document the sculptor could not repair.

Everything else in this application that can cost that much is already priced. A crossing is refused over a 512 MB budget by `Cost::within`; a hierarchy's next level is refused by `SubdivisionCost::within`. The two cheapest things to type were the two with no such check.

## What Changes

- A **field budget** is stated in the domain: how the brick cache is laid out (samples per brick axis, world units per sample) and what it may spend. A region is priced in bricks against it, exactly as a crossing is priced in cells.
- **Shape parameters are bounded by that budget** rather than by a constant. The bound is 4.08 — fifty-one bricks across, times the 0.16 world units a brick spans, halved because every parameter offered is a half-extent or a radius. It was 10.0, which is nearly two and a half times what the cache can hold.
- **Placing a form is priced before it is placed.** The region the shape's box fills is measured per axis and refused when the cache could not hold it, with a sentence naming both figures. The same price is paid by an insert into a layer, an insert as a subtool, and a re-measure of a form already placed, so no one of the three is the way round the other two.
- **A curve radius is priced the same way**, against the box the tube would fill, and refused before a single radius is written — so a refusal leaves the guide at the thickness it had.
- **A clamped parameter is reported**, with the value actually used, as a remark beside the answer rather than as a refusal: the placement happened, at a number the sculptor did not type.

Clamping each number is not a bound on the form, which is why both halves are here: a torus with both radii at the bound reaches four times the bound, and it is refused by the price rather than by the clamp.

## Capabilities

### Modified Capabilities
- `scene-objects`: placing a primitive, and re-measuring a placed one, are priced against the field and refused over it; the offered size range is derived from the cache rather than fixed.
- `sculpting-tools`: a curve's thickness is priced against the field and refused over it, and a refused thickness leaves the guide as it was.

## Impact

**Code**: `clayspace-model` (`field.rs`, new; `shape.rs`; `sculpt.rs`), `clayspace-engine` (`objects.rs`, `document.rs`), `clayspace-vm` (`object_vm.rs`), `clayspace-app` (one more remark channel).

**What a sculptor loses**: the top half of every size control. A form between 4.08 and 10.0 could be asked for before and could not be carried; asking for one now says so instead of spending the session finding out.

**What is priced and what is not**: the region an edit fills, against the bricks the cache could hold if every one of them held surface. That is the worst case on purpose — an edit whose region *could* come back all surface is one the cache may be asked to hold entirely. It is not a prediction of triangles, and it does not price a curve that grows by having points added far apart; that path is still bounded only by the cache's own refusal.

**Risk**: the bound is derived from `BrickConfig`, so re-tuning the cache moves it. `a_budget_describes_the_cache_it_prices` fails rather than letting the two drift.
