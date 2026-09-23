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
The safe wrapper SHALL map every `clay_result` code to `Result<_, ClayError>`. On
failure it SHALL capture the engine's thread-local detail message via
`clay_last_error` at the point of failure, before any further engine call can
overwrite it, and SHALL carry that message in the error value.

**Every code the engine's header declares SHALL become a kind that names it**, and
no two kinds SHALL print the same sentence. A code carried as an opaque number is
a refusal a caller cannot branch on and a reader cannot understand, and the
failure is silent: the call still returns an error, it simply says nothing useful.
A code the header does not declare SHALL be carried verbatim rather than
flattened into a neighbouring kind.

This SHALL be held by a check against the pinned header itself rather than by
review. The wrapper SHALL fail its own tests when the engine declares a result
code the table does not name, and SHALL skip that check — rather than failing it —
where the vendored source is not present, since a packaged build has the generated
bindings and not the engine's source tree.

Codes that are not `clay_result` values SHALL NOT be folded into the same kind.
The engine has refusal enumerations of its own, returned beside a result rather
than in place of one, and a refusal that means "this hierarchy has no such pass"
is not the same statement as "this call was malformed".

#### Scenario: Detail message is captured at the failure site
- **WHEN** an engine call fails and the application makes further engine calls
  before inspecting the error
- **THEN** the error still reports the detail message belonging to the original
  failure

#### Scenario: No panic across the boundary
- **WHEN** any engine call returns a failure code
- **THEN** the wrapper returns an error value and does not panic, abort, or unwind
  through the C boundary

#### Scenario: Every declared code has a kind of its own
- **WHEN** each code the engine's result enumeration declares is mapped
- **THEN** each becomes a distinct kind with a sentence no other kind prints

#### Scenario: A code the wrapper does not know is carried, not flattened
- **WHEN** a result code arrives that the wrapper's table does not name
- **THEN** the error carries the code itself rather than reporting a neighbouring
  kind

#### Scenario: A code added upstream fails the wrapper's own tests
- **WHEN** the pinned engine's header declares a result code the wrapper does not
  name
- **THEN** the wrapper's tests fail, naming the code

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

### Requirement: The pinned ABI and the container minor are constants a test holds
The safe wrapper SHALL name the engine ABI it was written against, and the
`.clayspace` container minor this build writes, as constants that a test checks
against the linked engine rather than against themselves.

Neither SHALL be derived from the linked engine. A constant read back from the
thing it is meant to check makes the check assert that a number equals itself,
and the whole value of both is that moving the submodule without moving them
**fails**.

The container minor SHALL carry, beside it, why this build writes that minor —
including that across a multi-release jump the minor may move in steps that no
single release's notes describe, so a reader meets the reasoning rather than
reconstructing it from one set of notes.

#### Scenario: The pin moves and the constants do not
- **WHEN** the vendored engine is pointed at a release whose ABI minor or
  container minor differs from the constants
- **THEN** the wrapper's own tests fail, naming both numbers and saying which
  one moved

#### Scenario: A file says what the build claims
- **WHEN** this build writes a `.clayspace`
- **THEN** the minor in the file's own header is the minor the constant claims

### Requirement: A document written now is refused by an older build, not misread
The container format SHALL fail in the direction that cannot corrupt work: a
build predating the pinned engine SHALL refuse a document this build writes
rather than read it as something else.

The application SHALL record which minor it writes where a person can find it,
because the consequence — a document that will not open elsewhere — is one a
sculptor meets and not one a maintainer does.

#### Scenario: An older build meets a newer document
- **WHEN** a build older than the pinned engine opens a document this build
  wrote
- **THEN** it refuses the file rather than reading the records out of step

### Requirement: The assembled surface's radial scale is reachable
The wrapper SHALL bind the engine's magnify of a layer's **assembled** surface,
with the centre, the signed strength, the region's radius and the easing the
engine's descriptor takes, and SHALL bind the preview that answers which nodes
it would warp without touching the document.

It is the radial counterpart to the assembled drag and exists for the same
reason: the per-item magnify deformer takes its centre in one item's local
frame, so on a form blended from several items it scales that item's field and
leaves the rest, with no error to show for it.

The strength SHALL be passed through signed and unscaled. It is a **total from
the start of a gesture** rather than an increment on the last frame — the
engine replaces its own last frame at a centre and radius it already holds —
and a wrapper that accumulated or rescaled it would break that idempotence.

The descriptor SHALL carry the radius and the easing and nothing else: a radial
scale has no direction to gate a half-space on, and no gesture identity to fold
on beyond the region itself.

#### Scenario: A blended form scales as one surface
- **WHEN** a magnify is applied at the join of two smooth-unioned items
- **THEN** both items take a warp and the surface moves on both sides of the
  blend

#### Scenario: The sign is the verb
- **WHEN** the same region is magnified at a positive strength and at a
  negative one
- **THEN** the surface swells away from the centre in the first case and
  gathers toward it in the second

#### Scenario: The frames of one gesture do not stack
- **WHEN** a gesture sends a growing total at one centre and radius over
  several frames
- **THEN** the field and the deformer chain are what a single call at the final
  total leaves

#### Scenario: Resolving is pure
- **WHEN** the preview is asked which nodes a magnify would warp
- **THEN** it names them and the document is unchanged, and the gesture
  afterwards warps the items it named

#### Scenario: What is not a gesture is refused
- **WHEN** a magnify is asked for at a strength of zero, which scales by one,
  or at a radius of zero, which is not a region
- **THEN** the call returns an error rather than recording a deformer that does
  nothing

### Requirement: Every result code the pinned header declares reaches a named kind
The wrapper SHALL name every `clay_result` code the pinned header declares, and
a test SHALL compare that vocabulary against the header on disk rather than
against itself.

A code the wrapper does not name SHALL still be carried verbatim rather than
guessed at, so an unknown result is reported as the number it was.

#### Scenario: The engine gains a result code
- **WHEN** a pinned engine declares a code the vocabulary does not name
- **THEN** the wrapper's own test fails and names the codes it found

#### Scenario: A replay refusal that changed nothing is told apart
- **WHEN** a journal is replayed onto a document that is not the snapshot it
  continues from
- **THEN** the refusal is reported as its own kind rather than as an unknown
  code, because it is the one replay refusal that leaves the document
  byte-identical and is therefore the one worth retrying with a different
  document

### Requirement: The full validation report is available, not only two bits of it
The wrapper SHALL expose the engine's whole mesh validation pass — the counts
as well as the booleans — and not only the watertight and manifold flags.

A caller told that a mesh is not manifold and not told **by how much** cannot
decide what to do about it: a handful of pinched edges in a large mesh and
thousands of them are different outcomes that the two-bit call reports
identically.

#### Scenario: A caller asks what is wrong rather than whether
- **WHEN** a mesh is validated
- **THEN** the number of non-manifold edges, boundary edges, degenerate and
  sliver triangles, and the Euler characteristic are available alongside the
  flags

### Requirement: A drag reports the region it invalidated
The wrapper SHALL offer the drag entry point that reports **where the gesture
reached**, and the host SHALL use it in place of reconstructing a region from
the brush size and the distance travelled.

A reconstruction cannot know what a layer fold above the layer can move, nor
what layers sharing an instanced edit list also change — both of which are
*under*-invalidation, which leaves stale surface on screen with nothing to
correct it.

The buffer MAY be sized from the layer's symmetry and re-asked on refusal,
because a buffer too small is refused before the first edit is recorded and
there is therefore no half-applied drag to unpick.

#### Scenario: A short buffer is refused rather than half-applied
- **WHEN** a drag is issued with a region buffer smaller than the gesture needs
- **THEN** the call is refused, the document is unchanged, and the count needed
  is reported

### Requirement: A layer's mirror is read rather than remembered
The host SHALL ask the engine what mirror a layer carries rather than keeping
its own account of what it last set.

A history step moves the engine's symmetry without telling the host, so a cache
can only be abandoned when it *might* be wrong — which costs an unnecessary
write on the next stroke and, where the answer is used to decide what to
re-fill, an unknown mirror that must be treated as all three axes.

#### Scenario: A layer that cannot carry a mirror
- **WHEN** the engine declines to answer for a layer whose representation
  cannot express a mirror
- **THEN** the host falls back to the safe over-invalidating answer rather than
  reading a refusal as "no mirror"
