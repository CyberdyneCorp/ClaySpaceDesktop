# Repair Mover Topológico: the bake is placed hard, and the band cannot hold it

## Why

Move Topológico on an SDF layer replaced a patch of the surface with a square
of visible stair-stepping, hard-edged against the untouched field (#128). The
gesture moved the surface by the right amount; the shading and the silhouette
were wrong.

Two earlier attempts read that capture as a verdict on the verb and proposed
withdrawing the tool. Both were wrong, and the measurement that refutes them is
one line long: **set the drag's displacement to zero** — at which point
`field::move_topological` returns `FieldVolume::sample(...)` before `solve()`
ever runs, so no geodesic is computed and no material moves — **and the tool
still scored 1.88x on the frame roughness that was being used to condemn it**,
against a bar of 1.5 and a control of 1.02x for the same bake placed without
the verb. Nine tenths of the defect was produced by a path with no geodesic in
it.

## What is actually broken, in two parts

**One. The verb throws the feather away.**
`clay_item_volume_move_topological` does not move a volume in place. It builds
a fresh one with `FieldVolume::sample` and swaps it into the item
(`clay_c.cpp:10896`), carrying over only the sample Lipschitz.
`FieldVolume::sample` takes no feather and `feather_` defaults to zero, so the
feather `ClayDocument::bake_volume` asked for is discarded, and our
`Op::Replace` lands as the hard replace that `VolumeParams::feather` and
`bake_volume` both describe in their own words — both fields live at the
boundary, branch-switching between them rippling the normals at the cell
wavelength. Suavizar, Relaxar, Planar and Polir never meet it because they
reach the engine through `clay_item_volume_relax_from` / `_flatten_from`, which
take `clay_volume_params` and set the feather themselves. There is no
`clay_item_volume_move_topological_from`.

**Two. The band was never wide enough to carry the drag.** Which is why simply
feathering the placement is not enough, and why the first version of this
repair silently ate the tool's whole reason to exist.
`clay_volume_params::feather` states the constraint: a feathered
`CLAY_OP_REPLACE` clamps its correction at the volume's **band**, "so a verb
that moved the surface further than the band from what sits beneath is
expressed only up to the band ... bake with a band that covers the verb".
`bake_volume`'s band is three cells — 0.06 — and this drag moves the surface by
the length of the gesture.

## What it is measured with

Two fixtures, both in the repository, both run on every one of the numbers
below.

`crates/clayspace-app/tests/visual_field_drag_quality.rs` renders the reported
stroke — brush 0.35, intensity 1.0, a three-sample 0.4 pull off the pole — on
every tool the shelf offers on a field and takes the mean
neighbour-to-neighbour pixel step over the lit pixels, as a ratio against an
untouched sphere rendered in the same run.

`a_topological_drag_leaves_behind_what_a_euclidean_one_carries` lifts one tip of
a horseshoe whose two tips are 0.5 apart in space and about 1.3 apart through
the material, and reports how far each tip came.

| path | frame roughness | near tip | far tip |
|---|---|---|---|
| bake and replace, the verb never called | 1.02x | — | — |
| bake, move, replace — as #128 shipped it | **2.07x** | +0.2933 | +0.0000 |
| the same with the displacement set to **zero** | **1.88x** | — | — |
| feathered, band left at three cells | *1.14x* | **+0.0601** | +0.0000 |
| feathered, band covering the drag | 1.45x | +0.2933 | -0.0002 |
| **and with a smootherstep weight — this change** | **1.38x** | **+0.2997** | **-0.0002** |

`+0.0601` is the band, to four digits: that row is what "expressed only up to
the band" looks like on a surface, and it is why the band is the other half of
the repair rather than a tuning knob.

Read that row's **1.14x** rather than its tip, and the frame guard prefers it
to the repair. That is not a flaw in the metric, it is the metric's subject: a
drag clamped to 0.06 barely moves the surface, and a surface that has barely
moved is smooth. So roughness cannot guard the band and displacement cannot
guard the feather — a hard replace is not clamped and lifts the tip the whole
+0.2933 while corrugating the shading. **Two guards, one per half**, and
reverting either half fails exactly one of them:

| reverted | `visual_field_drag_quality` | the horseshoe |
|---|---|---|
| the whole repair (`main`) | **FAILS** 2.07x | passes +0.2933 |
| the band only | passes 1.14x | **FAILS** +0.0601 |

The horseshoe's assertion was `> 0.05` when this change was first proposed,
which +0.0601 passes — so the band half shipped with no guard at all, and a
reviewer took it out with the suite still green. It is now two thirds of the
gesture.

## What the band costs

The first round of this change measured neither memory nor latency. Measured
now with `/usr/bin/time -l`, one stroke on the starting sphere, brush 0.35,
warm builds:

| pull | peak RSS, `main` → here | stroke, `main` → here |
|---|---|---|
| 0.4 — the issue's own stroke | 190 MB → **503 MB** | 51 ms → **111 ms** |
| 2.0 | 405 MB → **2225 MB** | 461 ms → **799 ms** |

It is the band, and it saturates. Holding the gesture at 0.4 so the sampled box
does not move and sweeping only the band gives 128 MB at 0.06, 388 at 0.26, 505
at 0.46 and 788 at 1.06 — then 683 at 2.06, no worse, because past the box's
own size every brick already stores samples. A volume stores samples in the
bricks its band reaches; a wider band is a thicker shell, until the shell is
the box.

It is **not** `padding`, which a review named as the cause and asked to have
pinned so the box would not grow with the drag. Pinning it is a measured no-op
(564 MB / 2333 MB, unchanged), and widening it alone with the band left at
three cells is also a no-op (127 MB / 347 MB). `clay_c.cpp:10471` says why:
`padding` is applied only on the branch that was passed no region, and this
call passes one. Nor does splitting the band between the two bakes help
(532 MB / 2431 MB) — they are sequential, so the peak is whichever is larger.

Nothing caller-side buys it back: the feather's clamp is on the correction's
magnitude, which *is* the displacement, so the band cannot be smaller than the
drag. The engine one-liner below removes the second bake and the wide band
together, and with them this entire cost.

## The engine's own fix, run rather than argued

The one-line change the mechanism predicts — `out.set_feather(v.feather())` in
`field/move_topological.cpp`'s `FieldVolume` overload — was applied to a local
build of the pinned engine and measured with **no caller-side repair at all**:
the same stroke reads **1.38x** and the horseshoe lifts **+0.2997**. Those are
this change's numbers to four digits. That is the fix to make upstream; what
lands here is the same result reached from the caller's side of a pinned ABI,
and it can be deleted the day the engine carries the feather through.

## What changes

`topological_move_stroke` bakes with a band that covers the drag, places the
volume the verb hands back, bakes the same region out of the document again
through a producer that *does* take `clay_volume_params`, and removes the hard
placement — leaving one feathered replace. The three engine edits are bracketed
so the stroke is one undo; ungrouped, the stroke spends four history entries
where a baked verb spends two, and undoing past the first puts the hard replace
back. `one_undo_takes_the_topological_drag_back_whole` holds that bracket, and
guards **this change's own internals rather than the reported defect**: `main`
makes a single `add_item` and passes it unchanged, so the test fails only when
the grouping is removed from the repair.

The weight's falloff moves from linear to smootherstep. With the feather live,
the kink at each end of `1 - g/radius` reaches the shading: measured, 1.45x
against 1.38x, and the anchored tip 0.2933 against 0.2997.

No tool is withdrawn, no verb row moves, and no count changes.

## What is left, and named rather than hidden

1.38x is the largest of any tool the shelf offers on a field — Mover reads
1.10x, Argila 1.31x — and it is the drag's own ripple: `g` is a Dijkstra
geodesic over a 26-neighbour lattice, quantised in steps of `cell`, `√2·cell`
and `√3·cell`. That is the next thing to ask the engine for. It is a tenth of
what was blamed on it, and it is now under the guard rather than over it.
