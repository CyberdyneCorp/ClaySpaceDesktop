## Why

A field layer's deformer chain only ever grows. Each Move dab appends one grab per mirror image, `safe_step_scale` is a product over the chain, so a worked patch decays geometrically and past a point the form stops rendering — at sixteen overlapping dabs a raycast found the surface 33 times out of 512. Issue #116 took most of the *cost* away; it did not bound the *growth*.

The cure has been in the ABI since ClayCore 0.73.0 and unusable until now. `clay_layer_consolidate_region` re-baked its own previous bake in full every time, so the closure ratcheted — twelve gestures on one patch went from 80 ms to 4098 ms for the same request. ClayCore #601 fixed that in v0.116.0, which this repository now pins, and the measurements below are ours on our own documents.

## What Changes

- A field layer consolidates the region a gesture touched **when that layer has degraded past a floor**, and not otherwise. The chain returns to zero; the next gesture starts from a single baked volume.
- `plan_region_merge` is called at the end of every field gesture to decide. It is pure and, measured upstream, costs **0.0002–0.0026 ms** — free against a 16 ms frame, and four ten-thousandths of the bake it decides about.
- The Optimize button stops refusing on a brush chain. `document.rs` currently returns *"esta camada está pesada por uma cadeia de pincéis"* because a layer-level action has no gesture to take a region from; it gains the last gesture's region and does the local bake instead.
- The host branches on `whole_layer` for local-vs-whole, and watches the returned `box` width against a fixed request as a ratchet detector. It does **not** branch on `absorbed == item_count`, which is a tautology on a one-root layer — and every document ships with one root until a sculptor adds a subtool.
- **No change to what a stroke does.** This runs after a gesture is committed, so it cannot alter the surface a sculptor just drew.

## Capabilities

### New Capabilities
- `chain-compaction`: when a field layer's deformer chain is collapsed into a baked region, what decides it, what it costs, and what a sculptor is told.

### Modified Capabilities
- `sculpting-tools`: the Optimize action's refusal on a brush-chain layer is replaced by a regional bake.
- `claycore-bridge`: `consolidate_region` and `plan_region_merge` move from bound-but-uncalled to load-bearing, with the signal rules above stated.

## Impact

**Measured on the starting sphere, one patch worked repeatedly, ClayCore v0.116.0, macOS Metal.**

| | chain, no merge | chain, with merge | bake cost | closure width |
|---|---:|---:|---:|---:|
| gesture 1 | 1 | 0 | 302 ms (absorbs the form, `whole_layer=1`) | 2.45 |
| gesture 4 | 4 | 0 | 110 ms | 2.88 |
| gesture 8 | 8 | 0 | 125 ms | 2.88 |
| gesture 24 | 24 | 0 | 123 ms | 2.88 |

Flat, not compounding — 139 / 130 / 109 / 123 ms at gestures 12 / 16 / 20 / 24, which is measurement spread rather than a curve. Unmirrored the same fixture costs 43–49 ms at width 1.12; the mirror roughly doubles both because the grabs reflect and `include_local_warps` folds both supports into the patch.

**We get the local path even with symmetry on.** ClayCore predicted otherwise — their gate refuses a node that participates in a layer mirror — but `set_symmetry` here sets the *layer* mirror while the starting form's volume does not participate. That was measured rather than assumed, and it is load-bearing: on the conservative path this would be whole-layer maintenance, which is measured **6x worse** for a deformer chain.

**Code**: `clayspace-engine/src/document.rs` (gesture commit, `consolidate_layer`), `clayspace-vm` (the status a sculptor sees), `claycore` (already binds both calls; no new FFI).

**Risk**: the floor is uncalibrated. It is the one number this change cannot take from an existing measurement, and the tasks below make calibrating it a step rather than a guess.
