# Move the engine pin to ClayCore v0.60.0

## Why

The pinned engine is v0.52.2. v0.60.0 is eight minor versions ahead in one tag
— 0.53.0 through 0.60.0, cut three days later — and its theme is the one this
application spends its interaction budget on: *a gesture costs what it
changes*. Measured against our own suite on this machine, the sculpting path
got materially faster and nothing in 1,389 tests broke.

**The pin moves cleanly.** The C ABI gained 27 entry points with nothing
removed and no signature changed, so the only source change the move forces is
`EXPECTED_ABI`, which `version_is_the_pinned_engine` holds to the submodule.
`clay_layer_info` grew two fields behind the `struct_size` it negotiates, and
this workspace writes that size from `size_of` of the compiled type rather
than by hand, so the growth is absorbed without a line changing.

**What the upgrade buys, measured here rather than quoted.** Two full
`just bench-compare` runs against the recorded 0.52.2 baseline, taking only
what moved in *both*:

| case | 0.52.2 | run 1 | run 2 | |
|---|---:|---:|---:|---|
| `dab.p95` | 4.16 | 2.20 | 2.21 | 1.88x |
| `subtool.solo.p95` | 21.46 | 12.58 | 13.37 | 1.61x |
| `locality.dab_ms` | 2.78 | 1.99 | 1.93 | 1.40x |
| `dab.median` | 2.10 | 1.62 | 1.68 | 1.25x |
| `mask.ungated.p95` | 2.75 | 2.27 | 2.26 | 1.21x |
| `history.undo.mean` | 87.26 | 77.01 | 77.08 | 1.13x |
| `tape.dab_after_96_edits` | 2.47 | 2.15 | 2.19 | 1.13x |

**Nothing regressed, and the second run is why that can be said.** The first
run reported `object.drag_frame_intersect.p95` at 76.11 ms against a recorded
49.59 — a 53% regression that would have matched what upstream admits about
this release, where #372 measured the unmirrored move path 1.09–1.12x slower
and their own device `sdf_move` case is 1.12x its v0.52.2 figure
(CyberdyneCorp/ClayCore#375). It was contention: that run shared the machine
with other work, and the second reads 49.99. Every flagged case came back
inside the spread — `object.drag_frame.p95` 28.03 then 19.72 against 20.77,
`frame.p95` 0.83 then 0.63 against 0.61, `subtool.boolean` 11,548 then 9,480
against 10,198.

**Filtered runs were the other false signal, and are not evidence.** A
`just bench-only` run measures a different shape — the justfile says so, and
it refuses to record a baseline for that reason. Asked for the dab group
alone it reported a median of 5.56 and 9.77 ms where the full run reports
1.62. Only full runs are compared here.

**The baseline is deliberately not re-recorded.** Re-recording hides whatever
regresses next, and the figures above are worth keeping as the thing a future
run is measured against. It stays at 0.52.2 with its conditions block saying
so.

## What changes

- The submodule pin, `EXPECTED_ABI`, and the documentation that states the
  engine version.
- A regression test for the one behaviour whose *contract* changed under us:
  `clay_layer_move_surface` now documents reflecting a drag into every image
  the layer emits, where v0.52.2's header said nothing about a layer mirror.
  This application reflects the gesture itself, so the hazard is a doubled
  pull. Measured: it does not double — a mirrored drag moves each side exactly
  as far as an unmirrored one — and the test is what keeps that true.

## What does not change

The capabilities this release opens are not taken up here. Node readback,
layer instancing, id-addressed payloads, live Smooth and Move transactions and
per-axis item scale each retire real workarounds in this codebase and each is
its own change with its own measurements. This one moves the pin and proves
the move is safe.
