# ClaySpaceDesktop

A 3D sculpting desktop application in Rust, rendering with WebGPU and driving
the [ClayCore](https://github.com/CyberdyneCorp/ClayCore) SDF + voxel engine
through its C ABI. macOS and Linux.

**Status: the engine bridge works; there is no window yet.** Group 1 of
`openspec/changes/add-clayspace-desktop` is complete — the whole engine surface
is reachable from safe Rust and covered by headless tests. The viewport arrives
in milestone 2.

## Prerequisites

| | Minimum | Why |
|---|---|---|
| Rust | 1.82 | workspace edition |
| CMake | 3.24 | ClayCore is a CMake project, built as part of `cargo build` |
| C++ compiler | C++20 | same |
| Network (first build only) | — | ClayCore fetches meshoptimizer, ufbx and xsimd at configure time |

Accelerated backends are optional and detected automatically. macOS picks up
Metal from the Xcode command line tools; Linux picks up CUDA (`nvcc` on `PATH`
or `CUDA_PATH`) and Vulkan (`VULKAN_SDK`, or `pkg-config`). **The CPU backend is
always compiled in**, so a machine with none of them still builds and runs.

## Getting it

The engine is a submodule, pinned to an exact commit:

```sh
git clone --recurse-submodules https://github.com/CyberdyneCorp/ClaySpaceDesktop.git
cd ClaySpaceDesktop
```

Already cloned without it? `git submodule update --init --recursive`. Forgetting
this is the most likely first failure, and the build says so by name rather than
letting CMake or the linker produce a worse message.

## Building and running

```sh
cargo build                 # configures and builds ClayCore, then the workspace
cargo test --workspace      # 45 headless tests; no display, no GPU required
```

The first build compiles the C++ engine and takes a few minutes. Later builds
only recompile it when the submodule's sources change.

### What runs today

```sh
# Which backends this build compiled in, and which this machine registered.
cargo run -p claycore --example diagnostics
```

```
engine version   : 0.26.0
expected ABI     : 0.26.0
compiled backends: metal
registered       : cpu, metal
```

The two lines answer different questions. *Compiled* is what the build selected;
*registered* is what the engine found at runtime. A backend can be compiled in
and still fail to register — a CUDA build on a machine whose driver is
unavailable, say — so only the second is the trustworthy one.

```sh
cargo run -p clayspace-app  # the composition root; prints its version and exits
```

`clayspace-app` is where the window will be built. It has no interface yet.

## Choosing backends explicitly

Detection is automatic, but can be overridden:

```sh
cargo build --no-default-features                 # CPU only, any platform
cargo build -p claycore --features metal          # require Metal
cargo build -p claycore --features cuda,vulkan    # require both
```

Naming a feature whose toolchain is missing is a **hard error** at configure
time, with the reason. That is deliberate: silently dropping it would produce a
binary that cannot register the backend you asked for.

Backend choice affects speed, never results. The engine holds every GPU backend
to 1e-4 relative against the CPU scalar reference, and
`every_registered_backend_agrees_with_cpu` asserts it here too.

## Layout

```
crates/
  claycore-sys/      generated FFI to clay.h; no hand-written declarations
  claycore/          safe wrapper — the only crate that calls claycore-sys
  clayspace-model/   domain; the only layer above that reaches the engine
  clayspace-vm/      ViewModels: observable state + commands, no egui/wgpu
  clayspace-view/    widgets and renderer; cannot reach the engine at all
  clayspace-app/     composition root
vendor/ClayCore/     the engine, pinned
openspec/            the specification this is built against
```

The dependency direction is one-way and mechanical: `clayspace-view` does not
depend on `claycore`, and `clayspace-vm` does not depend on `egui`, `wgpu` or
`winit`. Both are Cargo facts, so CI asserts them rather than review. `unsafe`
is confined to the two bridge crates; every other crate declares
`#![forbid(unsafe_code)]`.

## Working on it

This project is specified before it is built. The specification lives in
`openspec/` and is the source of truth for what the application should do.

```sh
openspec list                      # active changes
openspec show add-clayspace-desktop
openspec validate --all --strict
```

Implementation follows `openspec/changes/add-clayspace-desktop/tasks.md`.
Behaviour changes go into the specification first.

## Licence

MIT. ClayCore is MIT with an all-permissive dependency manifest, so static
linking imposes no copyleft obligation.
