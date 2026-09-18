# claycore-bridge Specification

## Purpose
The boundary between this workspace and the vendored ClayCore engine: where the
engine is pinned, how its FFI is generated, and the rules that keep `unsafe`,
raw handles and the size-query protocol on the engine side of the line rather
than spread through the application.
## Requirements
### Requirement: ClayCore is vendored at a pinned commit
The application SHALL vendor ClayCore as a git submodule at `vendor/ClayCore`, pinned to an explicit commit at or after the 0.26.0 ABI, which is the first to carry subset meshing, the brick apron and colour lattice, layout-directed vertex copy, tape export and device adoption. The pinned revision SHALL be recorded in the repository, and advancing it SHALL be a reviewed change rather than an automatic update.

#### Scenario: An engine older than the required ABI is refused
- **WHEN** the pinned submodule predates the 0.26.0 entry points the application depends on
- **THEN** the build fails at compile time on the missing or differently-shaped entry points, rather than at runtime

#### Scenario: Clean clone builds the pinned engine
- **WHEN** the repository is cloned with `--recurse-submodules` and built
- **THEN** the engine compiled is the pinned revision, and the build does not consult any network source for engine code

#### Scenario: Missing submodule fails with a stated cause
- **WHEN** `cargo build` runs with `vendor/ClayCore` absent or empty
- **THEN** the build fails with a message naming the submodule and the command that initializes it, not with a compiler or linker error

### Requirement: The engine is configured and built by the Rust build script
`build.rs` in `claycore-sys` SHALL configure and build ClayCore through CMake, selecting the preset from the target platform and probed toolchains, and SHALL link the result. The build SHALL emit `cargo:rerun-if-changed` directives covering the submodule's headers and sources so that an engine change triggers a rebuild.

#### Scenario: Preset follows the platform
- **WHEN** the build script runs on macOS with the Metal toolchain present
- **THEN** it configures the `metal` preset, and on Linux with a usable CUDA toolkit it configures the `cuda` preset

#### Scenario: No GPU toolchain still builds
- **WHEN** the build script runs on a machine with neither the Metal nor the CUDA toolchain available
- **THEN** it configures the `cpu-only` preset and the build succeeds

#### Scenario: Missing prerequisite is reported precisely
- **WHEN** CMake is absent or older than 3.24, or the C++ compiler does not support C++20
- **THEN** the build fails before configuring, naming the missing prerequisite and its required version

### Requirement: Raw FFI is generated, never hand-written
The `claycore-sys` crate SHALL contain only `bindgen` output generated from `vendor/ClayCore/bindings/c/clay.h` plus the build script. It SHALL contain no hand-written declarations, no logic, and no wrappers.

#### Scenario: Header change surfaces at compile time
- **WHEN** the pinned submodule is advanced to a revision whose `clay.h` changes a function signature the application calls
- **THEN** the regenerated bindings cause a compile error in the safe wrapper, rather than a runtime failure

#### Scenario: Descriptor struct sizes are honored
- **WHEN** the safe wrapper populates any versioned descriptor struct that carries a `struct_size` field
- **THEN** it sets that field from `size_of` of the generated type, so the engine's version check receives the size actually compiled against

### Requirement: Unsafe code is confined to the bridge
`claycore-sys` and `claycore` SHALL be the only crates in the workspace permitted to contain `unsafe` code. Every other crate SHALL declare `#![forbid(unsafe_code)]`, and CI SHALL fail if any crate outside the bridge contains `unsafe`.

#### Scenario: Unsafe outside the bridge fails CI
- **WHEN** an `unsafe` block is added to any crate other than `claycore-sys` or `claycore`
- **THEN** the build of that crate fails on its `forbid(unsafe_code)` declaration

### Requirement: Every fallible engine call becomes a Rust Result
The safe wrapper SHALL map every `clay_result` code to `Result<_, ClayError>`. On failure it SHALL capture the engine's thread-local detail message via `clay_last_error` at the point of failure, before any further engine call can overwrite it, and SHALL carry that message in the error value.

#### Scenario: Detail message is captured at the failure site
- **WHEN** an engine call fails and the application makes further engine calls before inspecting the error
- **THEN** the error still reports the detail message belonging to the original failure

#### Scenario: No panic across the boundary
- **WHEN** any engine call returns a failure code
- **THEN** the wrapper returns an error value and does not panic, abort, or unwind through the C boundary

### Requirement: Handle ownership is expressed in the type system
Owned handles SHALL be released exactly once by RAII wrappers. Borrowed handles
SHALL NOT outlive their owner, enforced by lifetimes rather than by convention.

Where the engine's own entry point takes a document together with a handle that
document lends — a mask attached to one of its layers — the safe wrapper SHALL
address that handle by the **identity of the layer it belongs to** rather than
lending it to the caller and asking for it back. A borrowed mask SHALL NOT have
to escape into a caller's mutation path for that caller to use it.

#### Scenario: A borrowed grid cannot be destroyed
- **WHEN** application code obtains a voxel grid from a document layer
- **THEN** the value it receives has no destroy method, and the compiler rejects any attempt to outlive the document

#### Scenario: An owned grid is released once
- **WHEN** a standalone voxel grid value goes out of scope
- **THEN** the engine's destroy entry point is called exactly once for that handle

#### Scenario: A borrowed mask cannot outlive its document
- **WHEN** code attempts to hold a document-owned mask past the document
- **THEN** it does not compile

#### Scenario: A layer's own mask gates an edit to that layer
- **WHEN** a mask is attached to a layer and a stroke is applied to the same
  layer naming that layer as the mask source
- **THEN** the frozen region is unchanged, and the caller never handled the mask

### Requirement: The size-query buffer protocol is wrapped once
Engine entry points that report a required buffer size when called with a null buffer SHALL be wrapped by a single shared helper that performs the size query, allocates, and performs the filling call. Individual call sites SHALL NOT reimplement the two-step protocol.

#### Scenario: Growing result between the two calls
- **WHEN** the required size reported by the query call is smaller than what the filling call needs because the document changed in between
- **THEN** the helper retries the query rather than truncating or over-reading, and returns an error if the size does not stabilize

### Requirement: The wrapper reflects the engine's stated thread-safety contract
A document value SHALL be `Send` and SHALL NOT be `Sync`. Concurrent reads SHALL be expressed through a snapshot reader that remains valid for the duration of its use, matching the engine's documented snapshot semantics. The batched evaluation entry point, which the engine documents as free-threaded against one const document, SHALL be callable from several threads through a shared reference.

#### Scenario: Concurrent readers agree with a single reader
- **WHEN** several threads evaluate and pick against one unchanged document at the same time
- **THEN** every thread receives the results a single-threaded caller would receive

#### Scenario: Mutation cannot race a reader
- **WHEN** application code holds a snapshot reader
- **THEN** the compiler rejects a concurrent mutable use of the same document

### Requirement: The bridge is verified against the engine without a window
The workspace SHALL include a headless test suite that exercises the bridge against the pinned engine: document creation, layer and item authoring, a stroke, a voxel sculpt verb, meshing, picking, save and reload. It SHALL run in CI on every supported platform and SHALL NOT require a display or a GPU.

#### Scenario: Headless suite runs on a CPU-only machine
- **WHEN** the bridge test suite runs on a machine with no GPU backend registered
- **THEN** every test passes using the CPU backend

### Requirement: The topological move is reachable
The wrapper SHALL bind the engine's topological move, which drags a volume with
a falloff measured along the material rather than through space, with the
anchor, reach, displacement and easing the engine's descriptor takes.

The moved volume SHALL be written back with the same feather every other
replacing edit uses. A hard `CLAY_OP_REPLACE` edge is a step in the field, and a
step in the field is a lattice on the surface — the inside is the volume, the
outside is whatever was there, and nothing crosses between them. With a feather
the band between the two crosses over, which is the whole of ClayCore #67.

#### Scenario: Reach is measured along the surface
- **WHEN** a topological move is applied to a volume whose two parts are close
  in space and far along the surface, with a radius smaller than the path
  between them
- **THEN** only the part containing the anchor moves

#### Scenario: A volume is required
- **WHEN** the move is applied to an item that carries no volume
- **THEN** the call returns an error rather than silently doing nothing

#### Scenario: A drag the move made leaves no lattice
- **WHEN** a drag is made on a field through the topological move and the
  surface is looked at
- **THEN** it carries no grid of steps at the edge of what the move replaced

### Requirement: A second vendored engine is pinned and checked the same way
The retopology and UV library SHALL be vendored as a git submodule at
`vendor/CyberRemesherAndUV`, pinned to a release tag, and advancing it SHALL be
a reviewed change rather than an automatic update — as the ClayCore pin is.

Two separate assertions SHALL hold it, because they answer two questions and a
library can satisfy one and still be wrong:

- The library's **declared ABI** SHALL be checked through the library's own
  compatibility call rather than by comparing two numbers in this workspace.
  The question is whether the library can serve a client compiled against
  *these* headers, which is not the question "are these two numbers equal".
- The library's **release number** SHALL be asserted against the pin, because
  the ABI check says the library can serve this build and says nothing about
  the submodule being the build we meant to pin.

Neither number SHALL be read back from the library it checks.

#### Scenario: The pin moves and the release assertion does not
- **WHEN** the submodule is pointed at a release whose version differs from the
  constant
- **THEN** the guard fails, naming both

#### Scenario: A library that cannot serve these headers
- **WHEN** the linked library declares an ABI its own compatibility rule
  refuses for the headers this build compiled against
- **THEN** the check fails rather than the workspace linking and finding out
  later

