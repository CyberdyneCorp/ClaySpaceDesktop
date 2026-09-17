# Validation

## Revisions and isolation
Baseline desktop: `895cac8`. Both applications use ClayCore `86f2ad9c` (PR #623), with the same default backend selection and release build. Candidate changes only fresh surface staging. The engine gitlink is not part of this change; the isolated checkout uses local vendor symlinks for testing.

## Correctness
Four unit regressions cover exact vertex attribute bytes, slot/index layout, degenerate tails, positive-only bounds, empty keys, incremental growth/relocation, and capacity refusal. Release results: application library 80 passed / 2 ignored; sculpt latency 4 passed; settlement 5 passed; visual sculpting 18 passed; end-to-end 3 passed initially and one failed, then the failed test passed on retry (111 distinct passing cases total).

The initial end-to-end failure at `agent_end_to_end.rs:692` received `consent_refused` instead of the expected timeout refusal text. Running the same unchanged test executable against the preserved main application also failed in the consent section, at line 691 (missing `gate`, a different assertion). The candidate then passed the focused retry. This establishes an unreliable baseline consent test in this environment, not reproduction of the exact initial assertion. Keep both failures and the successful retry visible; do not describe the initial full run as entirely green.

Formatting, whitespace, and all 53 strict OpenSpec validations pass.

## Maintainability
The cognitive-complexity skill does not support Rust (its report contains no records). Lizard Rust cyclomatic analysis is used as a labeled fallback: staging constructor 4, layout method 2, new regression tests 1–4. These are cyclomatic scores, not cognitive scores. Formatting and whitespace checks pass.

## Performance protocol
Ten alternating baseline/candidate application pairs, thirteen brushes per process, clean starting sphere restored by undo, fixed begin/continue/end actions. CPU affinity: 0,2,4,6,8,10,12,14. Wait for three samples with CPU idle >=75% and load average <5; monitor for sustained CPU contention and reject a busy run. No local builds or tests during timing. This does not eliminate driver, GPU or frequency noise.

Whole layouts now perform at most two nonempty queue writes. Vertex gaps are uploaded; padded index spans are preserved. Upload byte equality is therefore not expected for whole layouts, and extra bytes must be reported. Local patches retain the old write path.

Results pending. The 16 ms target is not established by this change.

## Preserved application artifacts
The paired executables are retained locally under `/tmp/clay-531-batch/{before,fixed}/clayspace-app`. SHA-256:

- Baseline: `4871e14665f25b5c5a6cc3cde9ed25181f92452eebfc86cb71bfaddd92c4cfbc`
- Candidate: `01d2d3d8398b5ce72636593927c3afb856e1403689bbfb2a92b23bae4b503d39`

Build: `CARGO_TARGET_DIR=/tmp/clay-531-host-target CARGO_BUILD_JOBS=4 LD_PRELOAD=/usr/lib/x86_64-linux-gnu/libstdc++.so.6 cargo build --release -p clayspace-app --bin clayspace-app`.

Tests: `cargo test --release -p clayspace-app --features agent-e2e --lib --test agent_end_to_end --test sculpt_latency --test settle_needed --test visual_sculpting --no-run`, followed by each produced executable with `--nocapture --test-threads=1`, real GPU/display, and `CLAYSPACE_AGENT_E2E=1`. End-to-end tests run the preserved default-feature application.
