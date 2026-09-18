## Why

ClayCore #531 reports a common 63–245 ms floor through the running application's MCP measurement. On current host main, a clean-sphere reproduction reports about 100 ms even for Mask and held brush samples. Session::measure unconditionally calls settle_geometry_now, which rebuilds the entire surface after the command already synchronized its dirty region. Session::settle also rebuilds before checking whether anything was pending. The meter creates work and wait is not idle when the application is idle.

## What Changes

Drain pending incremental geometry and an actually owed deferred settle, without forcing compaction. Report owed deferred work as outstanding and stop waiting when the interface thread cannot make progress. Add uploaded-byte deltas to measurement/wait reports so work can be distinguished from timing noise. Keep explicit full rebuilds and the existing stroke-end settle policy.

## Impact

Application composition root and additive MCP diagnostic fields. No engine ABI or pin change is required. This fixes an artificial shared measurement floor; genuine expensive region operations remain visible.
