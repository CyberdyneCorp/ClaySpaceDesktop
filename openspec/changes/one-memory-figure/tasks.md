## 1. Let the drawing count itself

- [x] 1.1 `clayspace_view::device_memory`: `DeviceMemory` (buffers, staging, targets), the shared `DeviceLedger` behind it, and `Resident`, which adds an allocation's bytes when made and takes them back when dropped.
- [x] 1.2 `Gpu::memory`, `note_frame_polled` and `note_device_idle`. `note_upload` counts staging as well as traffic; the renderer's end-of-frame poll and a capture's wait say what they collected.
- [x] 1.3 `GpuMesh` keeps a `Resident` beside each buffer, replaced with it on growth; the polyframe's index buffer the same.
- [x] 1.4 `Framebuffer`, the studio shadow map and `OffscreenTarget` count their textures, and the readback buffer, as targets.
- [x] 1.5 `SurfaceGeometry::resident_bytes` and `SlotMap::resident_bytes`: the CPU copy at capacity, with the tables beside it.

## 2. Put the cache in the ledger

- [x] 2.1 `BrickStats::bookkeeping_bytes`, which the C descriptor already carried and the wrapper dropped.
- [x] 2.2 `ClayDocument::memory_diagnostics` fills `cache_bytes` (payload and bookkeeping, and a live gesture's preview cache while one is drawing) and `cache_budget`.

## 3. One figure, in one place

- [x] 3.1 `MemoryDiagnostics` carries `cache_bytes`, `cache_budget` and `drawing: DrawingMemory`; `in_use` is the engine's total with both folded in.
- [x] 3.2 `clayspace_app::memory::ledger` and `drawing` assemble it; nothing else does.
- [x] 3.3 The meter reads the whole ledger once a second. The status area, the diagnostics report (built every frame, so it takes the meter's last reading rather than walking the cache itself) and `state.memory` all read that reading.
- [x] 3.4 The status bar shows the figure in use, and its bar is the cache against the budget, named on hover.
- [x] 3.5 `state.memory`: `in_use_bytes` is the whole; `parts` adds `cache`, `desenho` and the drawing's four; `footprint_bytes` beside it.
- [x] 3.6 The diagnostics report and the profile file carry the cache, the drawing and the figure in use.

## 4. Check it against the process

- [x] 4.1 `memory::footprint`: `/proc/self/status` on Linux, `top`'s footprint column on macOS, nothing elsewhere.
- [x] 4.2 `FootprintProbe` reads it off the interface thread every ten seconds; `FootprintWatch` logs a footprint past twice the figure in use plus half a gigabyte, with the breakdown, once per doubling.

## 5. Hold it

- [x] 5.1 `crates/clayspace-app/tests/memory_ledger.rs`: `the_ledger_counts_surface_geometry` (against a counting allocator), `the_ledger_counts_gpu_buffers`, `the_ledger_counts_staging_until_the_device_is_done_with_it`, `the_ledger_counts_capture_targets`, and `reported_memory_tracks_the_footprint` on the ten-times reference scene.
- [x] 5.2 `crates/clayspace-engine/tests/memory.rs`: `the_ledger_counts_the_brick_cache`.
- [x] 5.3 `clayspace-mcp` `report.rs`: `in_use_counts_the_cache_and_the_drawing`; `clayspace-model`: `the_figure_in_use_folds_in_the_cache_and_the_drawing`; `clayspace-view`: the `Resident` and staging gauge, and `the_memory_bar_is_the_cache_against_its_budget`; `clayspace-app` `memory.rs`: the watch and the footprint reader.
- [x] 5.4 `docs/features.md` and `docs/architecture.md` say what the figure in use counts.
