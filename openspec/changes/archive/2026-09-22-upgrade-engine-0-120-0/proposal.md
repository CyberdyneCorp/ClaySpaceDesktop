# Move the engine pin to ClayCore v0.120.0

## Why

The pinned engine is v0.116.0. v0.120.0 covers 0.117.0 through 0.120.0 and
none of the four was tagged on its own. Two shipped entry points answer
differently to a caller who changes nothing and recompiles nothing, and **no
version gate announces either**: there is no symbol to link against and no ABI
number that moved for them.

**The pin moves with no source change at all.** Thirteen symbols added, zero
removed, no struct re-laid out — `clay_brush_params` grew a trailing
`mask_threshold` behind the `struct_size` this workspace already writes from
`size_of`. The whole workspace compiled and the whole `claycore` suite passed
against v0.120.0 before a line of this repository was touched.

**One of the two host-visible changes reaches this application and one does
not, and which is which was measured rather than assumed.**

- **A mesh grab now reaches the whole drag** (ClayCore #624) — and it does not
  change a mark made here. The change is in the *stroke consumers*:
  `clay_mesh_sculptor_apply_stroke` and its siblings now capture the region
  once and carry it, where they used to re-gather it around the stroke's first
  sample on every stamp. This application does not send Grab through a stroke
  consumer. `stroke_mesh` and the hierarchy's stroke both make **one stamp at
  the anchor** carrying the whole gesture's displacement, for the reason those
  call sites already spell out: a resolved stroke walks the brush centre along
  the path, so a drag that leaves the surface reaches no material at all. The
  release notes are explicit that a caller driving `stamp` directly is
  unaffected unless it opens a carried region, and this one does not.
  `mesh_move.rs`'s seven cases pass unchanged, the drag still arriving within
  0.05 of `travel * intensity`.

- **A voxel grab's falloff now means the curve it is named after** (ClayCore
  #612) — and this one lands squarely on a sculptor's mark. See below.

**The four remaining changes add surface this application does not reach yet**
and are deliberately not taken up here: `clay_brush_params.mask_threshold`, and
the adaptive surface's undo, whole strokes and recorded strokes. Each is worth
its own change, and `mask_threshold` is the answer to a voxel-mask complaint
this repository has carried in prose since #139.

## The one that changes a mark: the voxel drag stops tapering

`clay_voxel_sculpt_grab` had been passing the `BrushFalloff` enum where a
`CEase` index was expected. The two do not line up, so **every falloff
delivered the next one's curve** and `Constant` was unreachable through the
grab. A grid drag in this application asks for `Constant` — `solid_footprint`
flattens the curve for every grid verb, because occupancy is binary and a
fractional weight can only be spent as scattered coverage — so what it actually
got was Linear, and the drag tapered by accident.

The falloff is not a coverage control for a drag. `clay_voxel_sculpt_grab` is an
inverse map: each cell in the ball samples from `p - displacement * w`, so `w`
shapes the **pull**. At 1 across the ball the neighbourhood translates rigidly,
which is a block being shoved; falling to the rim it draws the surface into a
bulge, which is what a Move brush is for.

`voxel_grab_taper.rs` was armed against exactly this and fired on the pin move.
Measured on its slab, brush 0.4 on a 0.05 grid:

| | rim rise | centre rise | drag of 0.05 / 0.10 / 0.20 / 0.40 / 0.80 |
|---|---|---|---|
| v0.116.0, asking `Constant` | +1 | +4 | 1 / 2 / 3 / 4 / **6** cells |
| v0.120.0, asking `Constant` | **+5** | +6 | 1 / 2 / **4** / **8** / **9** cells |
| v0.120.0, asking `Linear` | +1 | +4 | 1 / 2 / 3 / 4 / **5** cells |

So the drag asks for the curve it always wanted rather than the one it was
accidentally given. The mark a sculptor makes is the one that shipped, with one
difference measured rather than claimed away: a drag of 0.80 — twice the brush
radius, past where the bulge has already saturated — rises 5 cells where it rose
6.

**The named control is left honest.** `Borda` reaches a *masked* grid layer's
drag unchanged, because a masked layer keeps the engine's own weighting so the
mask goes on gating (#139). A sculptor who picks `Dura` there now gets a rigid
pull rather than the linear taper the old cast handed them, and one who picks
`Suave` gets a smoothstep rather than something closer to a Gaussian. That is
the control meaning what it says, and `docs/features.md` says so.

## What Changes

- **`EXPECTED_ABI` to 0.120**, by hand, held against the linked engine by
  `version_is_the_pinned_engine`. The container minor does not move, so
  `Document::FORMAT` stays where it is.
- **An unmasked grid drag asks for a tapering pull by name.**
  `dragging_footprint` is `solid_footprint` with the curve kept, and it is the
  only grid verb that wants one: for every other verb the falloff decides
  coverage, and for a drag it decides the pull.
- **`voxel_grab_taper.rs` gains a second reading of the same property**, at the
  centre rather than at the rim: a drag as long as the brush's own radius
  arrives short of the ask. The rigid pull carries the whole 8 cells; the
  tapering one carries 4.
- **`voxel_brushes.rs`'s drag starts where a press lands.** Its Mover case
  anchored half a ball inside the slab, which puts the slab's own surface where
  the weight is ~0 — those cells sample themselves and nothing moves. It read
  as a working drag only because the old cast's support reached a shade wider.
  `voxel_grab_taper.rs` records the same fixture fault and the same cure.

## What this change does *not* claim

The three tripwires the upgrade survey expected to fire did **not**, and they
were each run rather than reasoned about. `multires_document.rs` pins that a
dab on a hierarchy does not reach the `.clayspace` it was built from, and it
still does not; `mesh_automask.rs` pins that the cavity and surface-group
factors are accepted and inert from C, and they still are; and
`visual_voxel_sculpt.rs`'s tripwire is that the geometry revision is not a
document identity, which is about this application rather than about the
engine. v0.120.0 touches none of the three. The one that fired is
`voxel_grab_taper.rs`, which no survey had listed.

It also does not take up `mask_threshold`, the adaptive delta, or the adaptive
stroke calls. This application has no adaptive surface yet, and adopting a
stencil threshold is a change to what a mask means rather than a pin move.
