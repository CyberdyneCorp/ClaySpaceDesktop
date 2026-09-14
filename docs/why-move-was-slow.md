# Why the SDF Move brush was slow, and why it looked frozen

Two defects, reported as one. They had nothing in common except the tool they
were reported against, and separating them took longer than fixing either.

The report was: *"the performance of the Move brushes in SDF is not realtime"*,
and more precisely *"it takes almost a second after I do 3 or 4 move dabs in a
simple sculpture"*. Later, from the same sculptor: *"we only see the effect of
the move brush (in sdf) after the stroke finishes, not while I'm dabbing"*,
**with mesh mode behaving correctly**.

That second sentence is the one that mattered most, and it arrived last.

---

## Part one: it looked frozen

**Cause: one function asked the wrong question first.**

`SculptViewModel` sends *segments* while the pointer is down, so a surface
follows the hand instead of appearing on release. How long it waits between
segments was decided by `stamps_between_segments`:

```rust
fn stamps_between_segments(&self, tool: ToolKind) -> f32 {
    if self.model.active_representation() != Representation::Mesh {
        return STAMPS_PER_SEGMENT;      // three stamps' worth of travel
    }
    if self.replays_from_the_anchor(tool) { 0.0 } else { 1.0 }
}
```

It asked **what representation** before it asked **what kind of gesture**.

That threshold is correct for a *stamping* stroke. Each segment costs a re-mesh
of everything it touched, and the cost grows with the gesture, so sending one
per pointer move re-meshes the same neighbourhood over and over. Three stamps'
worth of travel is the pacing that makes a stroke affordable.

It is wrong for a *replayed* gesture. A drag is a displacement from where the
pointer went down, so the whole gesture is re-sent from its anchor every time
and the engine **replaces** the grab it already emitted rather than stacking
another. The work is identical on the first segment and the fortieth. Waiting
buys nothing and costs exactly what a sculptor sees.

**What that cost, in numbers.** At the default flow and a brush of 0.858, three
stamps' worth of travel is **1.03 world units** — most of the way across a unit
sphere. An ordinary drag ended before a single segment fired. Measured across
the fix on a 0.08-unit drag over eight pointer moves:

| verb | before | after |
|---|---:|---:|
| `Mover` | **0** segments before pointer-up | **8** |
| `Padrão` | 1 | 1 — the press's own dab, not a segment |

**And it starved machinery built for exactly this.** A field's Move has a live
transaction opened for it at press — `open_live_gesture` routes `Mover` into
`arm_live_move`, and `clay_sdf_move_begin` / `update` / `commit` exists
precisely to draw a drag while the pointer is down, previewing and taking the
preview back inside one segment so the undo depth never moves. None of it ran,
because nothing fed it a segment.

**Why it survived so long.** It was pre-existing, introduced with the change
that made *mesh* sculpting behave. The exemption was written for the mesh case
it was solving, and a field's Move only became a replayed gesture later — at
which point it qualified for the fast path and did not get it, because the
representation was asked about first.

**The tell was the asymmetry, not the slowness.** Mesh mode was correct
throughout. Two paths that should agree and do not is a far stronger signal
than either path being slow: "slow" needs a threshold before it is a defect,
"these two disagree" does not. Two repositories spent two days measuring
magnitudes and a sculptor found it in five minutes of actually dragging.

---

## Part two: it really was slow

Separate mechanism, separate fixes, several of them upstream.

### The floor: every SDF brush was over budget before anything degraded

A fourteen-brush sweep on a **clean one-item sphere**, first stroke, nothing
accumulated, put the *fastest* SDF brush at 4x over a 60 Hz budget. Move was
203 ms. So Move was never special — it is where the decay bites first, not
where the floor is.

The floor is brick count, and brick count is our voxel size:

| | `voxel_size` | brick span | bricks per unit³ |
|---|---|---|---|
| the engine's fixture | 0.05 | 0.4 | 1x |
| `ClayDocument::BRICK_CONFIG` | **0.02** | 0.16 | **15.6x** |

One press here dirties 288–2816 bricks where the same dab on the engine's
fixture dirties 64. Nothing is unexplained by that, and it is not a defect: 0.02
is what the application can sculpt at, and `dim: 8` is already the measured
optimum for it (16 was tried and cost 64 ms against 39 ms on the same edit).

### The field was being meshed twice per stroke

The larger host-side cost, and nobody had counted it.

On release, the viewport re-meshed the **whole field** through
`clay_document_mesh`. That was not a performance choice — it was hiding sliver
triangles the brick mesher emitted, whose face normals are cross products of
near-parallel edges and shade black. A whole-field mesh is a different mesher
and has none of them.

It cost twice over:

- A whole-field evaluation has **no region to cull against**, so the deformer
  cull cannot fire and it pays for every grab on the layer. Its cost tracks the
  *document* rather than the edit: 2.8 ms at one dab against 26.2 ms at 48,
  where the per-brick path goes 4.2 ms to 6.3 ms over the same span.
- Its output **cannot be patched incrementally**. The store held one mesh under
  a single key while the engine reports dirt per brick, so a flag forced the
  *next* edit to throw it away and rebuild every brick regardless — plus a full
  GPU relayout, 13.96 ms, where a per-key rebuild patches the slots it touched.

The engine stopped emitting slivers in v0.113.0, so the whole detour went, and
the second rebuild with it.

### The decay: every dab makes the next one slower

Each Move dab adds one grab **per mirror image**, and `safe_step_scale` is a
*product* over the chain, so it decays geometrically. Symmetry doubles the
rate and the starting form turns x on — which is why the report says *three or
four dabs*. Past a point the form stops rendering at all: at sixteen
overlapping dabs a raycast finds the surface 33 times out of 512.

Most of that decay was **bookkeeping rather than geometry**. The declared
Lipschitz bound grew 205x over 48 moves while the real gradient grew 2.1x —
a bound 99.9x more pessimistic than the truth, and the marcher takes its step
size from the declared one. Fixed upstream; picking on a worked form roughly
halved by moving the pin, with no code of ours involved.

### And one hardcoded literal was making it worse

`front_only: true` was written at every Move call site, so the near side of a
form always travelled alone and a form could never be dragged through.

It also degrades the layer. Eight dabs on a sphere leave a chain of 8 either
way, and `safe_step_scale` reads **0.2421 with the gate on against 0.5999 with
it off** — 2.5x less degraded for the same number of warps, enough to move the
layer's own health report from `Deformers` to `None`. A front-only grab gates on
the surface normal, so its weight field carries a discontinuity that the
declared bound must cover; a two-sided grab is smooth.

Nobody had questioned the literal, so nobody had measured it.

---

## What was ruled out, so it is not re-litigated

Five plausible explanations died on measurement:

| candidate | verdict |
|---|---|
| per-segment warp accumulation | already fixed; a gesture is one grab, not one per segment |
| `clay_sdf_move_preview_document` refactor | **1.02x** — measured, dead |
| tape-compile cache | 10–18%, inside noise |
| cull pad / batch union / spatial grouping | refuted; a 9.3x smaller union moves nothing |
| `point_the_mirror` per press | **0.00 ms** — it short-circuits |
| regional consolidation | closure is the connected component, and a sculpt is connected by construction |
| whole-layer consolidation | measured **6x worse** for a deformer chain |

---

## What the investigation itself got wrong

Worth recording, because the errors were more instructive than the fixes and
every one was caught the same way.

**The ledger could not be summed.** Every `timed()` label is an envelope that
may contain other timed labels — `begin stroke` contains `re-malha`, `stroke`
contains `re-malha final` — so adding them produces a number that means
nothing. Separately `FrameLog` keeps a max and a count and **never a sum**, and
records nothing under 16.667 ms. A "92.7 ms average" quoted from it was
tail-biased and should never have been used for attribution.

**A probe disabled the thing it was measuring.** `move_dab_cost.rs` passed
`0.0` for `advise_below_step_scale`, which *gates* the advice rather than only
setting its threshold. It read "never advises consolidation" at every step
scale down to 0.001308 and that was reported upstream as an engine defect. At
`0.5` the flag fires from the sixth dab unmirrored and the third mirrored.

**Two issues were filed on a misreading of our own code.** A claim that the
application re-sends drag slices from `applied - 1` was taken from reading
`pending()` without following its caller; `apply_segment` sends `whole()` for an
SDF Move and has since well before the investigation. A second issue argued that
one reconstructed box spans a mirror plane, when `mirrors()` already returns one
entry per image and the verb runs once for each. Both were corrected in the
open, and both had already reached the engine team as guidance.

**A constraint nobody checked shaped both teams' plans.** A claim that ~618
golden images needed refreshing after any engine pin was inherited, repeated
into three issues, a commit message, four messages to the engine team and their
release sequencing — and was never true. There are no golden images; the visual
suites write to a gitignored directory and assert properties, because a
pixel-exact golden fails on every driver. It survived because it was never the
thing under discussion, only the constraint being reasoned *from*.

**Three regression tests certified the bugs they were written to catch.** Found
by reverting each change underneath its own new test and confirming the test
fails. One pinned an ABI contract rather than our adoption of it; one asserted a
ratio bound that passed on the unfixed code; one compared a settled surface
against `clay_document_mesh` and passed only because `settle` *called*
`clay_document_mesh` — an output compared against itself, for as long as it had
existed.

The practice that found all three came from a measurement on the engine side: a
change that was wrong in one of three required parts produced **identical**
warp count, declared Lipschitz and step scale to the correct one, to every
digit, while the surface did not move at all. Cost metrics cannot see that. Only
the surface can.

---

## The shape of it

The slowness was real, mostly upstream, and mostly fixed by moving a version
number. The frozen-looking drag was ours, older than the investigation, and
cost one line — asking whether a gesture replays before asking what
representation it is on.

Neither was found by the instruments built to find them. Every one of those
measures what happens *after* a segment fires; the visible defect was a gate
deciding whether one fires at all.
