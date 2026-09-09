## ADDED Requirements

### Requirement: The second engine is built and shipped like the first
`cyberremesh-sys` SHALL configure and build the vendored engine through CMake
and generate its bindings, reporting a missing submodule, an insufficient CMake
or an unbuildable configuration before the linker produces a less legible
message — as `claycore-sys` already does.

The `cpu-headless` preset SHALL be what is built. The engine's shared object
exports only its `cyber_*` C ABI behind a linker version script, so the vendored
third-party definitions it links SHALL NOT be visible to or interposable by this
process.

Packaging SHALL carry the library and its licence notices; the engine reproduces
every third-party notice in a file that ships inside each release artifact, and
the packaging scripts SHALL include it.

#### Scenario: The workspace is packaged
- **WHEN** the packaging script runs
- **THEN** the retopology library and its third-party notices are in the artifact

### Requirement: The build cost of a second C++ engine is stated
Adding a second vendored engine to a workspace whose release CI jobs already run
for twenty to thirty minutes SHALL be measured and recorded rather than
discovered: the configure-and-build time it adds, and the size it adds to the
packaged artifact.

Where the cost is material, the change SHALL say so plainly rather than leave
CI to slow by an amount nobody attributed.

#### Scenario: The engine is added to the build
- **WHEN** the change is proposed for merge
- **THEN** the added build time and artifact size are recorded in it
