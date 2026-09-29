## 1. Release the staging

- [x] 1.1 `Gpu::flush_writes` (empty submission) and `Gpu::note_submitted`.
- [x] 1.2 `Renderer::set_mesh_layers` flushes its writes.
- [x] 1.3 The app's frame flushes its uploads when the window's image cannot be acquired (`SurfaceLoss::Skip | Reconfigure`).

## 2. Count it right

- [x] 2.1 `DeviceLedger`: open, flushed and submitted staging; `device_idle` releases only what was submitted.
- [x] 2.2 `OffscreenTarget::capture` marks its submission before it waits.

## 3. Measure it

- [x] 3.1 `cage.footprint` in the `cage` benchmark group, budget 1.2×; `Skip::NoFootprint` where the platform cannot read it.

## 4. Verification

- [x] 4.1 `device_memory.rs`: `a_wait_does_not_release_writes_no_submission_carried`, `a_flush_before_a_frame_is_counted_until_the_poll_after_next`.
- [x] 4.2 `tests/memory_ledger.rs`: `a_write_no_submission_carried_is_still_held_after_a_wait`, `a_hundred_carried_uploads_with_no_frame_leave_no_staging_behind` (fails with the flush removed).
- [x] 4.3 `bench` unit test `the_footprint_is_reported_beside_the_device_or_skipped`.
- [x] 4.4 `just bench-only cage` before and after, figures in the proposal.
