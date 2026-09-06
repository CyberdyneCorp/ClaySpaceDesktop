# Move the engine pin to ClayCore v0.84.0

## Why

The pinned engine is v0.78.0. v0.84.0 is six minor versions ahead in one tag —
0.79.0 through 0.84.0 — and where v0.78.0's theme was *a surface is something a
tablet can hold*, this one is narrower and older: **the engine stops paying for
what it provably cannot reach.** A uniform brick is not walked. A grab whose
ball a brick cannot touch is not compiled into that brick's tape. A sampled
volume is cropped to the region reading it. A layer moved rigidly is not
re-evaluated, because its surface afterwards *is* its surface before, moved.

**The pin moves cleanly, and that is a measurement rather than a hope.** The C
ABI gained **29 entry points with nothing removed, no signature changed and no
struct re-laid out** — counted from the two tags' own headers rather than taken
from the notes. One descriptor grew, `clay_sculpt_policy`, by three prefix-cache
knobs appended behind the `struct_size` it already negotiates, with all three
zero meaning today's behaviour exactly. The whole workspace compiled against
v0.84.0 with **no source change at all**.

**Three constants move, and each was held by a test that fired the moment the
pin did.** `EXPECTED_ABI` to 0.84; `Document::FORMAT` to minor 17; and the
paired assertion that a file this build writes says the minor this build claims.
That is the third release for which the upgrade notes' advice — write at the
older minor if you exchange documents with an older build — is **unreachable
from here**, for the reason recorded since v0.78.0: the minor is a parameter on
the C++ `serialize_document` and is on neither `save_clayspace` nor
`clay_document_save`.

**One brush changed what it measures, and the change is an improvement this
repository was leaning against.** Two tests failed asserting that every surface
brush moves a sphere by more than 1e-3; Suavizar moved it 3.0e-4. Measured
across the pin rather than adjusted to fit:

| | v0.78.0 | v0.84.0 |
|---|---:|---:|
| a **pick** against the resting unit sphere | 1.006707 | **1.000296** |
| what a smooth moves the **field** by, one stroke | 0.0400000 | **0.0400000** |
| what a smooth moves the **field** by, four dabs | 0.1198471 | **0.1198471** |
| what a **pick** reports for that same stroke | 0.0000000 | 0.0022205 |

**The brush is bit-identical across the pins and the pick is the entire
difference.** Read with `clay_eval_points` — a direct field read, no marcher and
no epsilon — over 2,197 points about the stroke, every point moves on both pins
and the deltas agree to the last digit. What changed is that v0.84.0 walks the
brick cache analytically instead of sphere-tracing it, so a unit sphere that
read 6.7e-3 off now reads 3e-4 off.

The fixture had been reading measurement error as clay: one stroke moves the
field by 0.04 and the old pick reported the surface as bit-identical,
`1.0350003` twice. A smooth of a *pristine* sphere correctly does almost
nothing, which is why the four region-sampling verbs are now given something to
sample.

**An earlier draft of this proposal said the brush was "stronger, not weaker",
on the strength of a pick reading of 0.008360 against 0.010919.** That measured
the instrument that changed. The control needed a control, and the field probe
is it.

**One thing this repository measured and filed is *partly* fixed, and the
difference between the release's claim and this fixture's answer is worth
carrying rather than smoothing over.** `benchmarks/ab-v0.73.0-vs-v0.78.0.md`
recorded a live intersecting boolean costing **1.166x** more on v0.78.0 than
v0.73.0, with a subtracting control on the identical fixture flat — reported
upstream as ClayCore #451, which v0.84.0's notes declare closed: *"the intersect
drag now costs what the subtract control costs."* Re-measured here on the same
machine, backend, viewport and scenes, four runs on a quieter box than the
original campaign ran on:

| | v0.73.0 | v0.78.0 | v0.84.0 |
|---|---:|---:|---:|
| subtracting, the control | 25.49 ms | 25.82 ms | **25.46 ms** |
| intersecting | 57.35 ms | 66.84 ms | **64.81 ms** |

**2.03 ms of the 9.49 ms came back — 21% — and the drag is still 1.130x what it
cost at v0.73.0.** That is not a contradiction of the upstream fix and must not
be filed as one: what was fixed is the layer *extent bound query*, and the
figures upstream publishes for it are sub-millisecond — 0.0669 ms a frame to
0.0003. A fix worth 0.067 ms was never going to account for 9.49. The quadratic
is gone and something else on this fixture is not, and the honest report says
both.

**What is confirmed is the pick.** `object.pick.ms` goes 0.113 → **0.055 ms**,
**0.49x**, against the notes' claim of 0.51–0.54x for `clay_raycast_attributed`.
This application calls that entry point directly, which is why the improvement
arrives without a line changing.

## What Changes

- The submodule points at v0.84.0; `EXPECTED_ABI` and `Document::FORMAT` follow.
- `sdf_brushes` gives the four region-sampling verbs something to sample. The
  engine adapter already names them together — "Suavizar, Relaxar, Planar and
  Polir do not stamp: they sample a region" — and that grouping is what the
  fixture was missing.
- The documented standing facts that this release changes are corrected where
  this repository asserts them, and the A/B report's open regression is marked
  as closed upstream with the figure that closed it.

## What this does not do

**It adopts none of the 29 new entry points, and this is the decision worth
recording rather than the omission worth hiding.** A pin move should be
separable from what the pin enables, so that a bisect over an upgrade lands on
the upgrade. Two of them are worth real work and are named here so the next
change has somewhere to start:

- **The layer placement gesture.** `place_layer` writes a transform and refills
  the union of the old and new bounds on **every frame of a gizmo drag**, and
  this application pays a second time to hide it — an SDF drag rebuilds the whole
  layer per frame so the mesher's artifacts never reach the screen. Upstream
  measures the engine's half at 12.4 ms per frame at 100 items and **95.7 ms** at
  1000, against 0.30 ms of matrix multiply, and
  `clay_layer_placement_begin`/`_update`/`_commit` makes sixty refills one.
- **The SDF prefix cache.** `clay_sdf_prefix_cache` with
  `clay_brick_cache_eval_requests_seeded` takes a cold brick from 14.65 ms to
  **0.291 ms** — flat across a ten-fold document. That is a hitch in the middle
  of a stroke, and it is the phase `a-profile-the-engine-team-can-read` now
  measures under its own name.

Also unclaimed, and smaller: stamp assets (`clay_layer_place_stamps` and its
capture), and two diagnostics that would sit beside the ones already exported —
`clay_document_extent_stats` and `clay_layer_warp_cost_get`.

## Capabilities

### New Capabilities
<!-- none: a pin move changes what the engine does, not what this application
     offers. What it enables is named above and specified by its own change. -->

### Modified Capabilities
- `claycore-bridge`: the ABI and container minor this build is written against,
  and what a document written now can be opened by.
- `sculpting-tools`: what a region-sampling verb is required to demonstrate, now
  that the surface it is measured against is measured accurately.

## Impact

- `vendor/ClayCore` (submodule), `crates/claycore/src/lib.rs` (`EXPECTED_ABI`),
  `crates/claycore/src/document.rs` (`FORMAT` and its reasoning).
- `crates/clayspace-engine/tests/sdf_brushes.rs`.
- `benchmarks/ab-v0.73.0-vs-v0.78.0.md`, `docs/roadmap.md` and `README.md` where
  they state a fact this release changes.
- **A document written by this build is refused by a build older than v0.84.0**
  rather than misread, which is the direction the format fails in by design.
