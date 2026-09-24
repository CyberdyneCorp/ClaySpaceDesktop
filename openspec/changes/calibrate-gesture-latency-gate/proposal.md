# Calibrate the gesture latency gate

## Why

The release `gesture_end` test compares a single worst segment and pointer-up against a fixed 16.7 ms wall-time threshold on every CI runner. A macOS CPU-only job on an unchanged commit measured a 23.5 ms segment, then passed on rerun. Shared-runner speed and contention decide the verdict.

## What changes

- Time a fixed full rebuild of the reference gesture scene in the same test process.
- Require each incremental segment to cost less than one quarter of that rebuild and pointer-up less than one tenth.
- Calibrate the related dab median and 95th-percentile CI gate to its own fixed-scene rebuild; the reference-machine 50/100 ms product targets remain documented.
- Keep printing raw times in debug and release, and assert the ratios only in release.
- Add a regression for the gate's scaling and its refusal of a lost incremental path.

## Impact

The CI gates measure whether the live paths stay incremental while absolute latency budgets remain product targets on a named reference machine. A shared runner's raw milliseconds remain visible in test output without deciding the result alone.
