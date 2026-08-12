# Everything you routinely do to this repository.
#
# `just` with no arguments lists the recipes. The point of this file is that
# the long-form commands live in one place: the test suite, the layering check
# and the specification gate are three different tools, and knowing to run all
# three should not depend on having read the README recently.
#
# https://github.com/casey/just

set shell := ["bash", "-uc"]

# The workspace binary, since the crate and the binary share a name.
app := "clayspace-app"

# List the recipes.
default:
    @just --list --unsorted

# -- building ----------------------------------------------------------------

# Debug build of everything. The first one compiles the C++ engine.
build:
    cargo build --workspace

# Release build, which is what every measurement here assumes.
build-release:
    cargo build --workspace --release

# Fetch the pinned engine. Forgetting this is the most likely first failure.
submodule:
    git submodule update --init --recursive

# Everything a fresh clone needs before it can do anything else.
setup: submodule build

# -- running -----------------------------------------------------------------

# Open the application.
run:
    cargo run -p {{app}} --release

# Open it with the CPU backend only, whatever the machine offers.
run-cpu:
    CLAYCORE_CPU_ONLY=1 cargo run -p {{app}} --release --no-default-features

# Versions, the engine revision, every registered backend and the active one.

# The same report the application's Ajuda → Diagnóstico window shows.
diagnostics:
    cargo run -q -p {{app}} --release --bin {{app}} 2>&1 | head -8

# -- testing -----------------------------------------------------------------

# The whole suite. No display and no GPU required.
test:
    cargo test --workspace --release

# One test target, e.g. `just test-one visual_brushes`.
test-one target:
    cargo test -p {{app}} --release --test {{target}} -- --nocapture

# Everything CI checks, in the order that fails fastest.
check: fmt-check layering lint test spec packaging
    @echo "all gates passed"

# Formatting, without changing anything.
fmt-check:
    cargo fmt --all -- --check

# Format in place.
fmt:
    cargo fmt --all

# Clippy over every target, warnings included.
lint:
    cargo clippy --workspace --all-targets --release

# The architecture rules: which crate may reach which, and where unsafe lives.
layering:
    python3 tools/check_layering.py

# The specification and the code have to describe the same application.
spec:
    openspec validate --all --strict

# Licence policy and advisories.
deny:
    cargo deny check

# The packaging scripts, and whether ATTRIBUTION.md is still current.
packaging:
    python3 tools/test_tools.py

# -- looking at it -----------------------------------------------------------

# These are meant to be looked at: several real defects were invisible to the
# assertions and obvious in the picture.

# Render every visual test and open the capture directory.
visual:
    cargo test -p {{app}} --release --test visual_brushes --test visual_shell \
        --test visual_armature --test visual_bake_tools --test visual_incremental \
        -- --nocapture
    @just open-visual

# Open the capture directory for this platform.
[private]
open-visual:
    @if command -v open >/dev/null; then open target/visual; \
     elif command -v xdg-open >/dev/null; then xdg-open target/visual; \
     else echo "captures are in target/visual"; fi

# Per-segment cost of every brush, which is what a sculptor feels as lag.
segments:
    CLAYSPACE_SEGMENTS=1 cargo test -p {{app}} --release --test visual_brushes \
        -- --nocapture --exact no_brush_stalls_the_stroke

# Where one stroke segment's milliseconds go.
budget:
    cargo test -p {{app}} --release --test stroke_budget -- --nocapture

# -- measuring ---------------------------------------------------------------

# Run the benchmark and print the table.
bench:
    cargo run -q -p {{app}} --release --bin bench

# Compare against the recorded baseline. This is the CI gate.
bench-compare:
    cargo run -q -p {{app}} --release --bin bench -- \
        --baseline benchmarks/baseline-macos-aarch64.json

# Do this deliberately: the baseline is what future runs are measured against,
# so re-recording hides whatever regressed since the last one.

# Re-record the performance baseline.
bench-record:
    cargo run -q -p {{app}} --release --bin bench -- \
        --json benchmarks/baseline-macos-aarch64.json
    @echo "baseline re-recorded — commit it with the reason"

# -- packaging ---------------------------------------------------------------

# Regenerate the attribution manifest from cargo metadata.
attribution:
    python3 tools/attribution.py

# Refuses to package a binary that links the engine dynamically, since the
# distributable is supposed to be self-contained.

# Build the distributable: a .app on macOS, a tarball on Linux.
bundle: attribution
    python3 tools/bundle.py

# -- the engine --------------------------------------------------------------

# Which ClayCore this build is pinned to.
engine:
    @git -C vendor/ClayCore describe --tags --long
    @git -C vendor/ClayCore log --oneline -1

# Deliberately a tag rather than a branch: a release is a thing that stays
# still, and their main is where they are still working.

# Move the engine pin, e.g. `just engine-pin v0.28.0`.
engine-pin tag:
    git -C vendor/ClayCore fetch --tags origin
    git -C vendor/ClayCore checkout --detach {{tag}}
    @echo 'pinned — rebuild, then `just test` and expect the repro tests to'
    @echo 'flip for anything this release fixed'

# -- housekeeping ------------------------------------------------------------

# Remove build output. The next build recompiles the C++ engine.
clean:
    cargo clean

# Remove only the Rust artifacts, keeping the compiled C++ engine.
clean-rust:
    cargo clean -p claycore -p claycore-sys -p clayspace-model \
        -p clayspace-engine -p clayspace-vm -p clayspace-view -p {{app}}
