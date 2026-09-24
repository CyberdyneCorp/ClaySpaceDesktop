# One memory figure, with what the application holds to draw in it

The application reported three different numbers for its memory and none of
them matched the process. In one audited session the status area read
**0.00 GB**, `state.memory.in_use` read **13 MB**, and the process footprint was
**26 GB**; in another, 225–359 MB against 0.9–3.84 GB. So the figure was
roughly 2,000x low in the first and 10x low in the second, and anything that
decides on memory — a budget refusal, an agent choosing whether to subdivide, a
sculptor deciding whether to keep working — decided on a number that was wrong.

## Why it happened

The ledger counted what the engine can walk and nothing else. `surface_ledger`
folds in the mesh sculptors and the subdivision hierarchies, and the engine's
report covers the document; everything this application allocates to *draw*
the document was in no figure at all:

- the per-key copy of the surface the viewport keeps (`SurfaceGeometry`);
- vertex and index buffers, at their capacity;
- the staging wgpu takes to carry each write, held until the device has
  finished with it — the dominant cost of the long session the GPU buffer fix
  was about;
- the render targets: the window's framebuffer, the studio shadow map, and a
  capture target for as long as it lives.

The brick cache was not in it either — it is held beside the document, as the
surfaces are — and the status area showed its payload alone, which is where
0.00 GB came from. The two figures were at least named apart by
`state-reports-enough-to-verify`; this is the change that proposal left for
later, which makes them one.

## What changes

- **The drawing counts itself.** `Gpu` gains a gauge beside its traffic
  counters. Every mesh buffer, framebuffer, shadow map and capture target takes
  a `Resident` when it is created, which adds its bytes and gives them back
  when it is dropped. Held beside the allocation, so the figure cannot outlive
  it or miss it. Staging is the bytes written in the frames the device may not
  have finished with: the frame in progress and the one the last poll followed.
  `SurfaceGeometry::resident_bytes` is the CPU copy, at capacity.
- **The cache is in the ledger.** The engine's diagnostics now carry the brick
  cache's payload *and* its per-key bookkeeping, with a live gesture's preview
  cache beside it while one is drawing, and the budget the document's cache was
  made with.
- **One figure, assembled in one place.** `clayspace_app::memory::ledger` folds
  the drawing into the engine's report. `MemoryDiagnostics::in_use` is the
  engine's total, the cache and the drawing. The status area's number, the
  diagnostics window and `state.memory.in_use_bytes` all read it through the
  same once-a-second meter, so a person and an agent cannot see different
  figures for the same moment.
- **The status bar's bar is the cache against its budget.** The number beside
  it is the whole; the bar is what the budget bounds. A bar that filled with
  the whole figure would read as the budget running out when what grew was the
  viewport's buffers, which the budget does not limit.
- **`state.memory` lists the parts.** `cache`, `desenho` (the drawing) and the
  drawing's four — `desenho/geometria`, `desenho/buffers`, `desenho/staging`,
  `desenho/alvos` — after the engine's own. `footprint_bytes` carries what the
  operating system charges the process, where it can be read.
- **A footprint the ledger does not explain is logged.** A probe reads the
  footprint off the interface thread every ten seconds. When it passes twice
  the figure in use plus half a gigabyte for the process at rest, the
  application logs it with the ledger's breakdown — once, and again each time
  it doubles. The 26 GB session would have been flagged as it passed 1 GB.

## Measured

On the ten-times reference scene built, meshed, uploaded and drawn (Apple
silicon, Metal): the ledger reports 473 MB — 0.6 MB engine, 17.7 MB cache,
166 MB geometry, 282 MB buffers, 7 MB targets — where the engine's figure alone
was 0.6 MB. The footprint grows by 690–710 MB over three runs, about 1.5x the
ledger, once the scene has been drawn and thrown away once; the first time a
scene that size is drawn it grows about 2.4x, the difference being pools the
allocator and the driver keep and reuse. A counting allocator in the test
binary puts the geometry the store reports within 3% of what its rebuild left
allocated.

## What this deliberately does not do

- **It does not promise `in_use` within 25% of the whole footprint.** The
  footprint also counts the code, the graphics driver, the interface, and
  freed memory the allocator has kept; none of those is a document's, and none
  is visible from inside the process without platform calls this application
  does not make (`unsafe` is forbidden in it). The invariant stated and tested
  is the growth one — a document's footprint within twice its ledger — and the
  logged one, with an allowance for the process at rest.
- **It does not count everything the interface allocates.** egui's own
  buffers and textures, and uniforms that live as long as the renderer, are
  small and constant; they are the process at rest.
- **It does not reclaim anything.** This makes memory visible; releasing it is
  the maintenance path's.

## Capabilities

### Modified Capabilities

- `diagnostics`: the report carries the figure in use, with the cache and the
  drawing folded in and listed, and a footprint the ledger does not explain is
  logged.
- `app-shell`: the status area shows the figure in use, and its bar the cache
  against the budget.
- `agent-observation`: `state.memory.in_use_bytes` is the whole figure, with
  its parts and the footprint beside it.
- `performance-budgets`: the budget bounds the brick cache, which is a part of
  the figure in use rather than all of it.
