# Tasks

## 1. Move the pin

- [x] 1.1 Point the submodule at v0.60.0 and move `EXPECTED_ABI`, which the
      `version_is_the_pinned_engine` test holds to the submodule
- [x] 1.2 Run the whole suite against the new engine and triage every failure
      against the release notes rather than assuming staleness — 128 test
      binaries, 1,389 cases, 0 failures, one ignored as before

## 2. Answer the contract that changed under us

- [x] 2.1 `clay_layer_move_surface` documents mirroring in 0.60.0 and said
      nothing about it in 0.52.2, while this application mirrors the gesture
      itself — measure whether the two compose into a doubled pull
- [x] 2.2 Hold the answer in a test: a mirrored drag pulls each side as far as
      an unmirrored one, which a doubled pull would fail and which comparing
      the two sides against each other cannot catch

## 3. Say what moved

- [x] 3.1 Compare the benchmark suite against the recorded baseline, in full
      runs rather than filtered ones — a filtered run measures a different
      shape and the justfile says so
- [x] 3.2 Take a second full run before believing either direction: the first
      reported a 53% regression on a live boolean drag that the second put
      back inside the spread, and the machine was shared during the first
- [x] 3.3 Record what improved in the roadmap, and leave the baseline
      unrecorded so what regresses next stays visible
