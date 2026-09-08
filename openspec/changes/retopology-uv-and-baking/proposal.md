# Retopology, UV and baking, through CyberRemesher

## Why

The pipeline is `sculpt -> retopo -> UV -> bake`. This application owns the
first stage and has nothing after it: a sculpt leaves here as triangles with no
edge loops, no UV layout and no maps. ClayCore is explicit that it will not grow
those — its quad path is a lattice grid its own header calls "the input a
retopology pass REPLACES, not the output one produces", and it is neither
watertight nor manifold.

**The seam was already negotiated and both engines built their half.** ClayCore's
roadmap records it in one line: *"their engine bakes, this one answers field
queries"*. ClayCore ships `clay_mesh_save_handoff` (ABI 0.52.0; we are pinned at
0.84.0). CyberRemesher ships the reading half, a `CyberFieldEvaluator` of three C
callbacks, and `cyber_bake_field` — whose field-sampled maps are *always empty*
for want of a volumetric engine to plug in. We are that engine, and we are the
host that neither side can write: `grep handoff crates/` returns nothing.

## What changes

CyberRemesher **v0.8.0** enters as a second vendored engine beside ClayCore, bound
the same way — a submodule, a `-sys` crate that builds it and generates raw
bindings, and a safe wrapper that is the only other place `unsafe` may live.
Above that seam it reaches the domain as ordinary Model traits.

Five capabilities, in the order their value and risk say:

1. **The handoff**, which needs no new dependency at all — bind ClayCore's
   existing writer so a sculpt can enter their CLI today.
2. **Quad retopology** — `cyber_remesh` over a mesh subtool, arriving as a new
   subtool rather than replacing one.
3. **Conform** — re-snap a retopologised mesh onto a sculpt that has moved
   since, preserving its topology and *reporting* the deviation.
4. **UV** — the automatic atlas, and the seam-path tool for the layouts where
   no traceable edge loop exists.
5. **Baking from the field** — normal, AO, curvature and cavity sampled through
   ClayCore's own `clay_eval_points` / `clay_eval_gradients` /
   `clay_measure_points`, which is the half only this application can supply.

## Sculpting performance is a constraint, not an aspiration

A second compute engine in the process is the risk this change carries, and the
budget is stated before any of it is written: **`dab.*`, `brush.*`,
`locality.*` and `tape.*` do not move.** Four decisions follow from that and are
specified rather than left to whoever writes the code:

- **CPU-only, by preset.** The `cpu-headless` build is what we link. ClayCore
  keeps the GPU; two engines contending for one CUDA device during a stroke is
  a latency problem nobody can debug from a frame time.
- **A bounded worker pool.** `cyber_set_max_worker_threads` is set at startup
  and never left uncapped. Their own note says an uncapped bake "takes every
  core the host is trying to share", and their cap is pinned by a test to be
  byte-identical to an uncapped run.
- **Off the interface thread, always.** Every call goes through the existing
  `clayspace-vm::jobs`, which already discards a result whose document moved on.
  Their cancellable entry points carry the progress and cancel callbacks that
  module's `Progress` and `Superseded` were written for.
- **Never during a gesture.** These are operations a sculptor asks for between
  strokes, and the refusal is stated rather than the timing hoped for.

## Why v0.8.0 and not v0.7.0

Two reasons, and the second is not about features.

**The ZRemesher track is in 0.8.0.** `zremesher`, the explicit `TopologyLayout`,
exact `--symmetry` as mirrored *connectivity*, topology guides and quality-scored
candidate selection are all in the tag, not after it. Planning against v0.7.0
would have shipped a plan that omits the capability this change exists for.

**v0.7.0 carries a defect we would be importing.** Their nightly hardening
workflow — ASan/UBSan, TSan, fuzzing, bench — was red on every lane, and had been
since *before* v0.7.0 shipped; push CI stayed green throughout, so it rotted
unobserved across two releases. Among what it was not catching: a PLY header
could size an allocation from an attacker-controlled count — an 823-byte file
requesting ~151 GB. **We import meshes.** That is a denial of service in any
build of ours that reads an untrusted file, and it is fixed in 0.8.0 with a
regression test and a fuzz corpus seed.

## What this change does not do
- **No routing rule of our own between quad solvers**, and no `--quality best`
  by default: their own note is that it costs a second full field solve, roughly
  doubling remesh time for a small quality gain. It is offered as a knob, and the
  cost is stated where it is offered.
- **No manual retopology toolkit.** The ~40 `cyber_retopo_*` calls, soft
  selection and stroke interpretation are a second interaction model — a
  different application, sharing a library. Named here so that leaving them out
  is a decision rather than an oversight.
- **No second UV or bake implementation.** Where their spec and ours disagree,
  theirs is right; that is the seam.
