## Context

ClayCore is a headless C++20 library. It ships a stable C ABI (`bindings/c/clay.h`, ~190 entry points) covering the whole engine: document and layer authoring, the item builder, the stroke engine, voxel grids and their sculpting verbs, mask fields, the brick cache, picking, meshing, evaluation and file I/O. Parity between the C ABI and `pyclay` is a CI gate in that repository, so the C surface is not a subset — it is the surface.

Three facts about ClayCore shape everything below:

1. **Backends are runtime-registered and parity-gated.** CPU is compiled in unconditionally and *defines correctness*; Metal, Vulkan, CUDA and OpenCL register only where available and are held to 1e-4 relative on distances against the CPU scalar reference. Backend selection is a `const char*` argument on each evaluation call, `NULL` meaning `"cpu"`. The app therefore does not need to write a backend abstraction — it needs a *policy* over one that already exists.
2. **The kernel dialect targets MSL, CUDA C, OpenCL C and C++ — not WGSL.** ClayCore ships its kernel headers precisely so hosts stop copying the math; `docs/06-host-gpu-previews.md` exists because ClaySpace's hand-written Metal preview used a smin of support `k` where `kernel/ops.h` uses `4k`, making every blend in the preview four times narrower than the real field. A WGSL raymarch that evaluates the *tape* would be that same bug, reintroduced in a language the shared headers cannot reach. A WGSL raymarch over *sampled bricks* would not — see Decision 3.
3. **The C ABI states its threading contract.** A document is safe to read from several threads at once and readers get a snapshot valid for the duration of their call; calls on a single mutable handle are the host's to serialize; the batched evaluation call is free-threaded against one const document.

The application must also honour a supplied visual design: a near-invisible interface, desaturated neutral ground so the material reads truthfully, floating tool trays, and skeuomorphism confined to the brush swatches and the pressure control. The stated style ratio is 60 minimalism / 20 skeuomorphism / 10 space-UI / 10 HUD, and the single warm accent exists to mark one thing — the active brush.

## Goals / Non-Goals

**Goals:**

- A sculptor can open the app, block out a form, refine it with the brush vocabulary, mask, layer, and export a watertight mesh — without the interface asking them to think about backends, bricks or tapes.
- Every visible sculpting verb maps to a real ClayCore entry point. No invented tools, no tools that quietly do something adjacent to their label.
- The app runs on macOS and Linux, using the best acceleration each machine offers, and produces byte-identical documents across all of them.
- The Rust/C++ boundary is crossed in exactly one crate, and unsafe code exists in exactly one crate.
- MVVM is enforceable by the compiler and by CI, not by convention.

**Non-Goals:**

- **Soft-body / physics dynamics.** The design's *Dinâmica* panel shows gravity (`-9.81`), rigidity and damping. ClayCore has no solver, its roadmap proposes none, and simulating clay is a research project rather than a panel. The panel ships with what the engine has: voxel size and the multi-resolution level stack (`clay_voxel_add_level` / `set_active_level` / `drop_level`), which is what "Dinâmica: Ligada" means operationally — resolution that follows the detail, ZBrush's Sculptris Pro rather than a physical simulation. Gravity/rigidity/damping controls are **not shipped disabled**; they are not shipped. Recorded as deferred so that a later change can reopen it honestly.
- **Windows.** Explicitly out of scope for this change.
- **Mesh-surface brushes.** ClayCore sculpts fields and voxels; it says so. Mesh layers are carried, saved and exported, never sculpted.
- **A WGSL field evaluator.** See Decision 3.
- **Collaboration, cloud, accounts, telemetry.** The app is local and makes no outbound connections.
- **Polygroups, slice/knife, USDZ.** Absent from ClayCore by deliberate decision on its side.

## Decisions

### Decision 1 — Vendor ClayCore as a submodule and build it from source via CMake in `build.rs`

`vendor/ClayCore` is a submodule pinned to a commit. `build.rs` selects the CMake preset from the target and from probed toolchains, builds a static library, and links it. `bindgen` generates `claycore-sys` from `bindings/c/clay.h` at build time, so an ABI change in the submodule is a compile error rather than a runtime surprise.

*Alternatives considered.* Prebuilt per-platform artifacts would build faster but require a release pipeline in a repository the app does not own, and would decouple the header from the binary — the one thing bindgen exists to prevent. Rewriting the engine in Rust discards a parity-gated four-backend implementation and its test suite. Using `pyclay` via an embedded interpreter adds a Python runtime to a desktop binary to reach a C ABI that is already there.

*Consequence.* CMake ≥ 3.24 and a C++20 compiler are hard prerequisites of `cargo build`. This is stated in the README and checked with a clear error, not a linker failure.

### Decision 2 — Two crates at the boundary: `claycore-sys` and `claycore`

`claycore-sys` is generated, `unsafe`, and has no hand-written logic. `claycore` is the only crate allowed to call it and the only crate permitted `unsafe` outside of `-sys`; it is enforced with `#![forbid(unsafe_code)]` in every other crate.

The safe layer encodes contracts the C header states in prose:

- **Ownership**: a `VoxelGrid` created standalone is owned and destroyed by the caller; one obtained from a document layer is borrowed and must not be destroyed. These become two distinct Rust types (`VoxelGrid` and `VoxelGridRef<'doc>`), so the header's "destroying a borrowed handle returns an error" is a case that cannot be written.
- **Errors**: every `clay_result` becomes `Result<_, ClayError>`, and the error carries the thread-local detail message read via `clay_last_error` at the point of failure — not later, when another call has overwritten it.
- **Buffers**: the size-query pattern (call with `NULL` to learn the size, call again to fill) is wrapped once, rather than at each of the dozens of call sites that use it.
- **Threading**: `Document` is `Send` but not `Sync`; concurrent reads go through a `DocumentReader` snapshot guard that matches the ABI's stated snapshot semantics; the batch evaluation entry point takes `&Document` and is free-threaded.

### Decision 3 — The viewport renders meshes from the brick cache's dirty set

wgpu draws triangles produced by ClayCore's own meshers. Interactive display uses the surface-nets preview mesher over the brick cache; export uses marching tetrahedra, which is watertight and 2-manifold by construction.

The path this depends on exists as of ClayCore 0.26.0 (issue #43, PR #52): `clay_brick_cache_mesh` now takes a `keys_xyz` + `key_count` subset and returns per-key `clay_brick_mesh_range` values, so a dab re-meshes only the bricks its influence bound dirtied and patches the matching sub-ranges of the GPU buffer. Their benchmark: **22.6 ms** to re-mesh 232 surface bricks against **0.64 ms** for the 8 a dab dirties. Vertex upload uses `clay_mesh_copy_vertices` with a `clay_vertex_layout`, writing our interleaved layout in one pass into a mapped buffer.

*Alternatives considered.* Two raymarch routes, both now real:

- **Over the tape** (`clay_tape_export`, added in the same release) — analytic, exact, no meshing at all. Requires the kernel dialect in WGSL, which the shared headers cannot generate, which is the drift bug above. Still deferred, and the precondition is unchanged: `clay_parity_fixture_json` asserted per-case against the WGSL evaluator in CI before it draws anything a user could mistake for the document.
- **Over sampled bricks** — a brick DDA with trilinear sampling of the fp16 lattice, which is what `clay_brick_cache_raycast` already does on CPU. This contains *no kernel math*, so it carries no drift risk, and ClayCore's `docs/06` now opens by recommending it as the cheaper of the two host routes. `read_bricks` gained an `apron` parameter and an opt-in RGBA8 colour lattice specifically to make it work.

We take meshing for v1 because the subset path is measured inside our latency budget and needs no sparse-atlas machinery on our side. The volume route is the first thing to try if `performance-budgets` shows meshing dominating, or if polygonization artefacts become visible at working resolutions; it removes meshing from the interaction path entirely.

*Consequence.* Brush feedback latency is meshing latency, bounded by the dirty set. One caveat ClayCore documented rather than engineered away: meshed vertices are welded on canonical lattice-edge keys and the weld **spans brick seams**, so a triangle in one key's index range may reference a vertex in an earlier key's vertex range. Sub-ranges may be overwritten — which is what makes incremental upload work — but not freed in isolation. Buffer compaction is therefore a periodic whole-mesh operation, not a per-dab one.

### Decision 4 — Backend policy, not backend abstraction

At startup the app calls `clay_list_backends` and ranks the result against a fixed per-platform preference:

| Platform | Preference order |
|---|---|
| macOS | `metal` → `cpu` |
| Linux | `cuda` → `vulkan` → `opencl` → `cpu` |

The `vulkan` backend has been registered in ClayCore since before we specified this and implements the full interface, so the Linux non-NVIDIA hole this ranking was originally written around does not exist: an AMD or Intel GPU gets Vulkan, not OpenCL's best-effort subset. CUDA stays first on NVIDIA as the more mature tier-2 path; if measurement shows Vulkan matching it, the two swap and nothing else changes.

`cpu` is always present, so the list is never empty and startup cannot fail for want of a backend. Because some backends report `Unsupported` for some operations by design — OpenCL does not implement raycast, whose sphere-tracing utilities are templated C++ that OpenCL C cannot compile, and implements no device meshing — the policy is **per operation**, not per session: an `Unsupported` result is a fallback to CPU for that call, recorded once, never an error shown to the user.

The chosen backend is displayed and overridable, because a user debugging a suspected GPU problem needs to force the reference path, and because ClayCore's own parity guarantee makes that switch observably free of consequence to the result.

### Decision 5 — MVVM with the layering enforced mechanically

```
crates/
  claycore-sys/     generated FFI                     (unsafe)
  claycore/         safe wrapper                      (the only unsafe consumer)
  clayspace-model/  the domain: tools, interfaces, types. NO engine dependency.
  clayspace-engine/ the ClayCore-backed implementations of those interfaces
  clayspace-vm/     ViewModels: observable state + commands. No egui, no wgpu.
  clayspace-view/   egui widgets + wgpu renderer. Reads VM state, emits commands.
  clayspace-app/    composition root, window, event loop
```

The rule that makes this checkable: `clayspace-vm` does not depend on `egui` or `wgpu`, and `clayspace-view` does not depend on `claycore` or `claycore-sys`. Both are Cargo dependency facts, so `tools/check_layering.py` asserts them in CI rather than review.

**Why the domain and the engine adapter are separate crates.** The first attempt put both in `clayspace-model`, and the layering check failed on its first run: `view → vm → model → claycore` reaches the engine transitively, which the isolation rule forbids. There is no arrangement of the other crates that fixes that while the domain and engine access share a crate, so they were split. Only the composition root depends on `clayspace-engine`.

The benefit is not only purity: the ViewModel tests build and run without compiling the C++ engine, which is the difference between a fast feedback loop and a slow one.

egui is immediate-mode and has no data binding, so "ViewModel" here means an explicit state struct plus a command channel, not observers. A View function is `fn(&SculptViewModel, &mut Ui) -> Vec<Command>`: it may read ViewModel state and may emit commands, and it has no other way to affect anything. That is testable without a window, which is the property MVVM is being asked for.

*Alternatives considered.* Slint offers real property binding and a more literal MVVM, but embedding a custom wgpu viewport is less proven there than in egui, and the viewport is the application. Iced's Elm architecture is clean but is a different pattern wearing MVVM's name.

### Decision 6 — v1 keeps the brick cache and pays one host copy; device injection is deferred

ClayCore 0.26.0 added device interop: `clay_device_adopt` takes our `MTLDevice` or `VkDevice` as a `void*` under a `clay_device_api` tag — no vendor header reaches `clay.h` — and `clay_brick_cache_eval_requests_device` / `clay_eval_grid_device` write evaluation output straight into our buffers. wgpu can yield the underlying device through `wgpu-hal`, so this is available to us on both platforms.

We do not take it in v1. The header states the trade-off, and it is not a performance trade-off:

> generations, staleness, band classification, fp16 quantization and the memory budget are host code over host memory […] If you want the cache's correctness, use `clay_brick_cache_read_bricks`; if you want no host copy, use this. **Both are complete paths; neither is both.**

The device path bypasses the brick cache, so we would reimplement dirty generations, staleness, LOD, the memory budget and fp16 quantization ourselves. ClayCore notes that a second quantization implementation "is the thing most able to drift from us" — the same argument that produced this whole design. Trading a bounded memcpy for a reimplementation of the component whose correctness we are relying on is the wrong direction at this stage.

So: `read_bricks` and `clay_mesh_copy_vertices` in v1, one copy on the dirty-brick path only. Device injection becomes its own change when `performance-budgets` shows the copy dominating — and per Decision 4's platform split it would land on Linux first, since ClayCore reports Vulkan adoption verified and **Metal adoption compiled but never run on Apple hardware**.

*Alternatives considered.* Exporting shared allocations rather than sharing a device was the other half of what we asked for; ClayCore declined it deliberately, on the grounds that sharing an allocation needs external-memory extensions, matching physical devices and a per-API handle lifetime the ABI would own, where sharing a device needs none of those. That reasoning holds, and it means there is one interop path rather than two.

### Decision 7 — Brush names in the interface are a translation table, not new semantics

The design labels tools in Portuguese. Each label binds to a documented ClayCore verb, using that engine's own ZBrush-equivalence table as the authority:

| UI label | ClayCore verb | C entry point |
|---|---|---|
| Padrão | `Op::Relief` along a stroke | `clay_layer_apply_stroke` |
| Inflar | `Op::Relief` / `sculpt_inflate` | `clay_voxel_sculpt_inflate` |
| Suavizar / Relaxar | `field::relax` / `sculpt_smooth` | `clay_item_volume_relax`, `clay_voxel_sculpt_smooth` |
| Mover | `brush::move_brush` | `clay_layer_move_surface` |
| Puxar | `brush::snakehook` | `clay_item_create` + `clay_item_set_curve_points` |
| Pinçar | `magnify` (negative) / `sculpt_pinch` | `clay_voxel_sculpt_pinch` |
| Raspar | `sculpt_scrape` | `clay_voxel_sculpt_scrape` |
| Planar / Polir | `field::flatten`, cut-only mode | `clay_item_volume_flatten` |
| Preencher | `sculpt_fill_cavities` | `clay_voxel_sculpt_fill_cavities` |
| Nudge | `sculpt_smudge` | `clay_voxel_sculpt_smudge` |
| Camada | stroke preset, clamped accumulation | `clay_stroke_preset_*` |
| Máscara | mask field stroke | `clay_mask_apply_stroke` |
| Trim | `cut::cut_item` | `clay_cut_create` |

Where a verb exists on one representation only — `sculpt_carve_alpha` is voxel-side, `field::flatten` requires a region on the SDF side — the tool is disabled with a stated reason on layers that cannot accept it, rather than silently doing nothing. ClayCore is explicit that many verbs can be valid calls that change nothing (a sub-cell grab, a stamp that misses every cell); the app distinguishes a dead edit from a live one with `clay_voxel_change_count` deltas rather than by result code.

## Risks / Trade-offs

- **ClayCore is young and moving fast (0.26.0, first commit 2026-08-03), and its roadmap places "make it sculptable" in Phase 1.** → The submodule pin is the stability boundary; upgrades are deliberate, reviewed changes with the parity fixture re-run. The app's spec states which engine capabilities it depends on, so a breaking upgrade surfaces as a spec conflict rather than a bug report. 0.26.0 already carries one announced ABI break — four `clay_brick_cache_*` entry points gained parameters rather than `_colored`/`_apron`/`_subset` siblings — which is an arity change, so it fails at compile time and cannot be misread.
- **Meshing latency is brush latency** (Decision 3). → The brick cache dirty set bounds work to the influence bound; `performance-budgets` sets a measured ceiling; surface-nets preview is used interactively and marching tetrahedra only on export. If the ceiling is missed, the brick-volume raymarch is the recorded escape route.
- **Incremental mesh buffers fragment over a long session**, because welded vertices span brick seams so a key's range can be overwritten but not freed. → Compaction is a periodic whole-mesh re-mesh off the interaction path, budgeted in `performance-budgets` rather than left to grow.
- **CUDA on Linux may be absent, mismatched, or newer than the toolkit.** ClayCore's own README notes an RTX 50-series card against CUDA 12.0 falls back to PTX-only with driver JIT. → Backend probing is runtime, Vulkan is a full-interface second tier on the same platform, CPU is always present, and the app reports which backend it actually got rather than which it hoped for.
- **Metal device adoption has never run on Apple hardware** — ClayCore reports it as compiled and CI-checked but unexercised, and told us so rather than letting us find out. → Decision 6 defers device injection entirely, so v1 does not depend on it. When it is taken up, Linux/Vulkan leads and macOS follows behind a hardware test.
- **`bindgen` regenerating against a moving header can silently change safe-wrapper assumptions.** → The safe layer has its own test suite against the pinned submodule, and CI fails on a bindgen diff that is not accompanied by a wrapper review.
- **egui's immediate mode invites putting logic in the View.** → Enforced by the dependency rule in Decision 5: the View crate cannot reach ClayCore even if someone wants it to.
- **The design's *Dinâmica* panel promises something the engine does not do.** → Addressed head-on in Non-Goals rather than shipped as dead controls. This is a design-vs-engine conflict the user should confirm, not one the implementation should paper over.
- **The style ratio is a real constraint, not decoration.** 20% skeuomorphism confined to brush swatches and the pressure control means the rest of the chrome must stay flat; drift toward "richer" panels breaks the stated intent ("estou modelando barro, não operando um software"). → `design-system` states the budget as testable requirements.

## Migration Plan

Greenfield; nothing to migrate. Rollout is by milestone, each independently runnable:

1. **Bridge** — submodule, build, FFI, safe wrapper, backend probe. Verified headless by a test that loads a `.clayspace` and evaluates points on every registered backend.
2. **Viewport** — window, wgpu, MatCap, camera, a static mesh from a document. First visible milestone.
3. **Sculpt loop** — one brush, stroke engine, dirty-brick re-mesh, undo. The latency budget becomes measurable here and is measured here.
4. **Shell** — panels, scene tree, layers, inspectors, design system.
5. **Full tool vocabulary, IO, packaging.**

Rollback at any milestone is `git revert`; there is no deployed state and no data format the app owns (`.clayspace` is ClayCore's).

## Open Questions

- **The *Dinâmica* panel.** Confirmed as resolution controls (voxel size + level stack), with gravity/rigidity/damping dropped? Or is soft-body simulation a genuine product requirement, in which case it is a separate research-scale change with no engine support behind it today?
- **Localization.** The design is Portuguese throughout. Ship pt-BR only, or pt-BR + en-US from the start? The `app-shell` spec assumes externalized strings either way, so this is a scope question rather than an architecture one.
- **Voxel-first or SDF-first default document.** ClayCore carries both, and several verbs exist on one side only. The specs cover both representations; which one a new document opens as decides what the first-run experience feels like.
