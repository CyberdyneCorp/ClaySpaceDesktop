## Why

[ClayCore](https://github.com/CyberdyneCorp/ClayCore) is a headless C++20 SDF + voxel sculpting engine with a stable C ABI, four evaluation backends (CPU / Metal / CUDA / OpenCL) and a complete sculpting vocabulary — 28 primitives, 14 deformers, extended combine ops, a stroke engine, armatures (ZSpheres), mask fields, 10 voxel sculpting verbs and watertight meshing. It has no desktop application: it is "the engine core of ClaySpace (iPad sculpting app)" and otherwise reaches users only through a CLI and Python bindings.

ClaySpaceDesktop is that missing application — a Rust desktop sculpting tool that renders with WebGPU, drives ClayCore through its C ABI, and picks its acceleration backend from the machine it happens to be running on. The engine already guarantees that backend availability *changes speed, never results*; the app's job is to expose that vocabulary through an interface a sculptor can stay inside, not a control surface for a library.

## What Changes

- **New Rust workspace** (`crates/`) implementing a macOS + Linux desktop application, built on `wgpu` (WebGPU) for rendering and `egui` for the interface chrome.
- **ClayCore vendored as a git submodule** at `vendor/ClayCore`, configured and built by `build.rs` through CMake with a platform-selected preset, with `bindgen` generating raw FFI from `bindings/c/clay.h`.
- **Two-crate FFI split**: `claycore-sys` (raw, unsafe, generated) and `claycore` (safe Rust — RAII handles, `clay_result` → `Result`, borrowed-vs-owned handle typing, thread-safety markers matching the C ABI's stated contract).
- **Runtime acceleration selection**: probe `clay_list_backends`, rank by platform (macOS → `metal`, Linux → `cuda`, then `opencl`, then `cpu`), fall back silently and correctly, and surface the active backend to the user.
- **MVVM architecture** enforced as a layering rule: Model (ClayCore + domain) → ViewModel (observable state + commands) → View (egui, pure function of ViewModel state). No ClayCore type crosses into the View layer.
- **Mesh-based WebGPU viewport**: ClayCore's surface-nets mesher feeds interactive display via the brick cache's dirty set; marching tetrahedra produces watertight geometry for export. No SDF math is reimplemented in WGSL.
- **The interface described by the supplied design**: dark neutral shell (`#23262B` / `#3A3E45`), a single warm accent (`#D9744A`) reserved for the active brush, humanist sans labels with mono numerics, brush shelf, scene tree, layer stack, brush/material inspectors, view presets, navigation gizmo and a memory meter wired to the brick cache's real budget.
- **Document lifecycle**: `.clayspace` open/save, OBJ/PLY/FBX/glTF import and export, autosave and crash recovery, undo/redo with stroke coalescing.
- **NOT INCLUDED — soft-body dynamics.** The design's *Dinâmica* panel shows gravity, rigidity and damping. ClayCore has no physics solver and none is planned in its roadmap. See Non-Goals in `design.md`; the *Dinâmica* panel ships covering what the engine actually has (voxel size and multi-resolution levels).

## Capabilities

### New Capabilities

- `claycore-bridge`: Vendoring, CMake-driven build, generated FFI, and the safe Rust wrapper — handle ownership, error mapping, thread-safety, and the size-query buffer protocol.
- `gpu-acceleration`: Runtime backend discovery, platform ranking, per-operation capability fallback, user override, and the guarantee that backend choice never changes results.
- `mvvm-architecture`: The Model/ViewModel/View layering contract, the command bus, observable state, and the testability rules that follow from them.
- `viewport-rendering`: The wgpu render pipeline, MatCap shading, incremental re-meshing from the brick cache dirty set, camera, view presets, navigation gizmo, grid and brush cursor.
- `sculpting-tools`: The brush shelf and tool vocabulary mapped onto ClayCore verbs, brush parameters, the stroke engine, symmetry, masking and the cut/trim tools.
- `scene-and-layers`: The scene tree, the layer stack, visibility/ghost/lock protection, layer transforms and reordering, and selection driven by ClayCore picking.
- `document-io`: `.clayspace` documents, mesh import/export, recent files, autosave, crash recovery and units.
- `edit-history`: Undo/redo, stroke coalescing, undo groups and history presentation.
- `design-system`: The visual language — palette, style ratio, typography, accent discipline, iconography, spacing, and the skeuomorphic budget.
- `app-shell`: Window chrome, menu bar, panel layout and persistence, status bar, memory meter, localization.
- `performance-budgets`: Interaction latency, frame-rate, meshing and memory targets, with the measurements that verify them.
- `build-packaging`: Toolchain requirements, feature flags, CI matrix for macOS and Linux, and distributable bundles.

### Modified Capabilities

None — this is a greenfield project with no existing specs.

## Impact

- **New**: the entire `crates/` workspace, `vendor/ClayCore` submodule, `build.rs` CMake integration, CI workflows for macOS and Linux.
- **External dependency**: ClayCore (MIT) at a pinned commit. It is young (v0.22.1, first commit 2026-08-03) and under active development; its own roadmap places "make it sculptable" in Phase 1. The submodule pin is the app's stability boundary.
- **Build toolchain**: CMake ≥ 3.24 and a C++20 compiler become hard requirements for `cargo build`. macOS additionally needs the Metal toolchain; Linux optionally needs the CUDA toolkit (absent → CPU, which is always compiled in).
- **Licensing**: ClayCore is MIT with an all-permissive dependency manifest, so static linking imposes no copyleft obligation.
- **No server, no network, no user accounts.** The application is entirely local; it opens and writes files the user chooses and makes no outbound connections.
