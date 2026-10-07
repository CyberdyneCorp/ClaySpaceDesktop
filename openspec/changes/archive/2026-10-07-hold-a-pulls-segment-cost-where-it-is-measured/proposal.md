# Hold a pull's segment cost where it is measured

## Why

`a_long_pull_keeps_its_segment_cost` delivers the audit's 60-sample Snake Hook pull segment by segment and asserted that the median of its last ten segments stays under 6x the median of segments 2–11. On the hosted macOS runners it failed in four of thirty jobs between 29 Sep and 7 Oct — 6.51x on main's Metal debug job (run 36658810228), 6.69x on a CPU-only release job (37571178170), 7.31x on a Metal release job (37572423835), 7.51x on a Metal debug job (36615009192) — on code the other twenty-six jobs passed, and it passes on a developer Mac. The figures behind those ratios are 0.93–1.80 ms early and 6.82–13.55 ms late: a segment's cost grows mildly with the path on every machine, because every brick of the tip evaluates the whole tendril, and grows more the more loaded the host is, because that work is parallel and a late segment has more of it to wait for. A quotient of two figures that small measures the runner's scheduler, and a gate that fails on the runner's noise is one people learn to ignore.

## What Changes

- The test drains the dirty set before every segment, as the viewport does every frame, and holds the bricks a segment dirties as the exact assertion: a late segment dirties the tip's region (125 bricks against 150 early on the starting sphere), never the tendril's (421 by the end of the pull). This is deterministic on every machine and backend, and it is the assertion the quadratic class cannot pass.
- The wall time is held beside it as a class bound, fastest of five takes: the late median stays under six times the early one or under the audit's own cheapest segment, 24 ms, whichever is larger. The floor is 1.8x the worst runner reading and twelve times the developer Mac's; the audit's late segment is three times over it.
- The test remains a verdict in every profile on every machine. Nothing is ignored, and nothing is reported without being asserted.
- `docs/features.md` and `docs/why-move-was-slow.md` carry the measured figures and the rule; `sculpting-tools` states that a segment of a pull dirties its tip and what the cost gate referees.

## Measured

Apple M3 Pro (Metal, v0.126.0, sharing the host with other builds), the test as it was: early 0.46–0.62 ms, late 0.70–2.06 ms, 1.34–4.51x over ten release runs; 0.66–0.69 ms and 0.87–1.35 ms, 1.31–1.95x over three debug runs. A warm take with the host loaded read 0.25 ms against 1.58 ms, 6.3x. Hosted `macos-14` runners, the four failures: 1.05 / 6.82 ms, 1.13 / 7.53 ms, 0.93 / 6.82 ms, 1.80 / 13.55 ms.

The test as it stands, fastest of five takes on the same Mac: early 0.40–0.47 ms, late 0.67–1.60 ms, 1.54–3.73x over ten release runs; 0.56–1.34 ms and 0.75–2.92 ms, 1.36–2.18x over three debug runs; 150 and 125 bricks on every run.

## Impact

Test and documentation only. No behaviour changes. `a_stroke_on_a_grown_layer_begins_near_the_first`, the sibling 16x gate in the same file, passed every one of the same thirty jobs and is left as it is.
