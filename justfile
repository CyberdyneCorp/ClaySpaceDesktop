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
    cargo run -p {{app}} --release --bin {{app}}

# Forces a rebuild of the engine, because it is the C++ configure step that
# this changes and not a Rust feature. `--no-default-features` would do
# nothing: every crate here declares `default = []`.

# Open it with the CPU backend only, whatever the machine offers.
run-cpu:
    CLAYCORE_CPU_ONLY=1 cargo run -p {{app}} --release --bin {{app}}

# The application prints this on the way up and Window → Agent address and key
# shows it, but a terminal is where a client's configuration is usually being
# written.

# Where a client connects, for the session that is open now.
agent-access:
    @cat "${XDG_STATE_HOME:-$HOME/.local/state}/clayspace/agente.acesso" 2>/dev/null \
      || cat "$HOME/Library/Application Support/ClaySpaceDesktop/agente.acesso" 2>/dev/null \
      || echo "nothing is listening: no agente.acesso in the session directory"

# The menu does the same thing while the application is running. This is for a
# machine where it should not open at all.

# Shuts the agent door for the next session.
agent-shut:
    @mkdir -p "${XDG_STATE_HOME:-$HOME/.local/state}/clayspace" 2>/dev/null || true
    @printf 'fechada\n' > "$HOME/Library/Application Support/ClaySpaceDesktop/agente.porta" 2>/dev/null \
      || printf 'fechada\n' > "${XDG_STATE_HOME:-$HOME/.local/state}/clayspace/agente.porta"
    @echo "the agent door will stay shut until it is opened from the Window menu"

# The agent-facing crate's own suite. No display, no GPU, no engine built.
test-agent:
    cargo test -p clayspace-mcp

# Starts a real application with a window of its own and contends with the
# visual suite for the adapter, so it is asked for rather than run by `just
# test`. One test target, on its own, single-threaded. Debug rather than
# release, unlike the rest: what this checks is protocol behaviour, and a
# second whole-tree build to check it would cost more than it is worth.

# The door, driven against the real application over loopback.
test-agent-e2e:
    CLAYSPACE_AGENT_E2E=1 cargo test -p {{app}} --features agent-e2e \
      --test agent_end_to_end -- --test-threads 1 --nocapture

# What the engine reports about itself, without opening a window. The
# application's own Ajuda → Diagnóstico adds the graphics adapter and this
# session's fallbacks to the same picture.

# Which engine is linked, and which backends it registered.
diagnostics:
    cargo run -q -p claycore --example diagnostics

# Ajuda → Exportar perfil… writes a JSON file for the engine's authors: the
# distribution behind every phase of a stroke, the conditions it was taken
# under, and the shape of what was being sculpted. An unoptimised build runs
# that work about two and a half times slower, so the file it writes declares
# its own timings incomparable — this recipe is the way to produce one that
# does not have to.

# The application, optimised, so a profile exported from it can be quoted.
profile:
    cargo run -q -p {{app}} --release

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
    # `-D warnings` because CI lints that way. Without it `just check` passed
    # on a tree whose lint job was already red on main, which is the one thing
    # this recipe exists to prevent.
    cargo clippy --workspace --all-targets --release -- -D warnings

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

# The targets are read off the directory rather than listed here. A
# hand-written list rendered five of the twenty-five, and what it left out was
# the newest work: `visual_objects` was the capture that caught a blank first
# frame and a row of sliders labelled with save-file keys, and it never ran
# under this recipe.

# Render every visual test and open the capture directory.
visual:
    cargo test -p {{app}} --release $(for target in crates/clayspace-app/tests/visual_*.rs; do echo --test $(basename $target .rs); done) -- --nocapture
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

# The whole table takes several minutes: it measures every brush on every
# representation, every layer operation, the eight conversions, the
# subdivision hierarchy, consolidation, export, repair, masking and undo, and
# several of those rebuild a reference scene between samples. `just bench-only
# <prefix>` is there for when a question is about one group — `multires`,
# `normals` and `maintenance` are the three newest and each takes seconds.

# Run the benchmark and print the table.
bench:
    cargo run -q -p {{app}} --release --bin bench

# A filtered run refuses to record a baseline: a baseline recorded from a
# subset reports every omitted figure as missing on the next comparison.

# One group of it, e.g. `just bench-only brush.voxel` or `just bench-only convert`.
bench-only prefix:
    cargo run -q -p {{app}} --release --bin bench -- --only {{prefix}}

# One baseline per platform. Comparing a Linux run against a macOS recording
# measures the difference between two machines and calls it a regression, which
# is worse than no gate: the figures differ by more than any change would.

# Compare against the recorded baseline for this platform. This is the CI gate.
bench-compare platform=os():
    cargo run -q -p {{app}} --release --bin bench -- \
        --baseline benchmarks/baseline-{{ if platform == "macos" { "macos-aarch64" } else { "linux-x86_64" } }}.json

# Do this deliberately: the baseline is what future runs are measured against,
# so re-recording hides whatever regressed since the last one. And record it on
# the platform it is named for — the file's `conditions` say which machine and
# which engine produced it, and a mismatch there is the first thing to check
# when a comparison looks wrong.

# Re-record the performance baseline for this platform.
bench-record platform=os():
    cargo run -q -p {{app}} --release --bin bench -- \
        --json benchmarks/baseline-{{ if platform == "macos" { "macos-aarch64" } else { "linux-x86_64" } }}.json
    @echo "baseline re-recorded — commit it with the reason"

# -- measuring across two engine pins ----------------------------------------

# An engine upgrade is measured against the pin it replaces, and neither side
# of that measurement is the committed baseline: the A side is a build of this
# application against the old engine, which is not this working tree. So the
# two recipes below take a path rather than deriving one. Reaching for
# `bench-record` instead is the mistake they exist to prevent — it writes over
# `benchmarks/`, so an A/B run would silently replace the file every future run
# is judged against, with a number from an experiment.
#
# The shape of an A/B: record the old pin's side from its own checkout with
# `bench-to`, come back here, and `bench-against` that file. Do both on a quiet
# machine — a run recorded beside other work stays wrong for every comparison
# after it — and do each side several times rather than once, because the
# variance between two runs of an unchanged tree is larger than the spread
# inside either of them. The file now carries that spread, so a change landing
# inside the baseline's own range is marked in the table.

# Record a whole run to a path of your own, leaving the committed baseline alone.
bench-to file:
    cargo run -q -p {{app}} --release --bin bench -- --json {{file}}

# The comparison is *permitted* across engine pins and announced above the
# table rather than refused: refusing would leave an upgrade with no instrument
# at all, and the conditions carry both the engine version and the vendored
# submodule's revision, since two builds can both say 0.78.0 and differ by a
# commit. Every figure key added on this side and absent on the other reports
# as `new` and cannot be compared — that is not a fault, it is what a new
# measurement is.

# Compare this build against a baseline recorded elsewhere, e.g. another pin.
bench-against file:
    cargo run -q -p {{app}} --release --bin bench -- --baseline {{file}}

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

# `claycore-sys` is deliberately absent: its build script *is* the C++ engine
# build, so cleaning it is what costs the several minutes this recipe exists
# to avoid. Use `just clean` when you want that.

# Rebuild the Rust crates, keeping the compiled C++ engine.
clean-rust:
    cargo clean -p claycore -p clayspace-model -p clayspace-engine \
        -p clayspace-vm -p clayspace-view -p {{app}}
