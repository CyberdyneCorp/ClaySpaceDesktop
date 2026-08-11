## 1. Workspace and engine bridge

- [x] 1.1 Create the Cargo workspace with crates `claycore-sys`, `claycore`, `clayspace-model`, `clayspace-vm`, `clayspace-view`, `clayspace-app`; add `#![forbid(unsafe_code)]` to every crate except the first two
- [x] 1.2 Add ClayCore as a git submodule at `vendor/ClayCore` pinned to a commit at or after the 0.26.0 ABI; record the revision and document the clone instructions
- [x] 1.3 Write `claycore-sys/build.rs`: check CMake ≥ 3.24 and C++20 support with named errors, select the preset from target and probed toolchains, build and link the engine, emit rerun-if-changed for the submodule
- [x] 1.4 Generate bindings with `bindgen` from `vendor/ClayCore/bindings/c/clay.h`; verify no hand-written declarations exist in `claycore-sys`
- [x] 1.5 Implement the error layer in `claycore`: `clay_result` → `Result`, capturing `clay_last_error` at the failure site
- [x] 1.6 Implement the size-query buffer helper once, and the descriptor-struct helper that sets `struct_size` from `size_of`
- [x] 1.7 Implement owned vs borrowed handle types (`Document`, `VoxelGrid` / `VoxelGridRef<'doc>`, mask, mesh, brick cache) with RAII release on the owned side only
- [x] 1.8 Implement the threading markers: `Document: Send + !Sync`, the snapshot reader guard, and the free-threaded batch evaluation entry point
- [x] 1.9 Wrap the authoring surface: document, layers, groups, items, deformers, blends, transforms, mirrors, repetition
- [x] 1.10 Wrap the sculpting surface: stroke engine and presets, move brush, snakehook, cut tool, field relax/flatten/move-topological, consolidation
- [x] 1.11 Wrap the voxel surface: grids, resolution levels, all ten sculpt verbs, fills, palette, repair, `change_count`
- [x] 1.12 Wrap masks: create, paint, stroke, fill, invert, invert-within, expand, contract, smooth, to-field, extrude
- [x] 1.13 Wrap the brick cache: config, dirty marking, request drain, submit, readback (with apron and optional colour), surface bricks, stats, LOD mips, subset mesh with per-key ranges, single and batched raycast
- [x] 1.13a Wrap the layout-directed mesh copy (`clay_mesh_copy_vertices` / `_copy_indices`), rejecting layouts that name absent or overlapping attributes
- [x] 1.14 Wrap picking, meshing and file I/O
- [x] 1.15 Write the headless bridge test suite covering authoring, a stroke, a voxel verb, meshing, picking, save and reload; make it pass with no GPU and no display

## 2. Acceleration policy

- [x] 2.1 Implement backend discovery over `clay_list_backends` returning the parsed list
- [ ] 2.2 Implement the platform preference ranking (macOS `metal` → `cpu`; Linux `cuda` → `vulkan` → `opencl` → `cpu`) and automatic selection
- [ ] 2.3 Implement per-operation fallback: route an `Unsupported` result to the CPU backend for that operation, keeping the selected backend active elsewhere
- [ ] 2.4 Record each fallback once per operation kind in the diagnostics log
- [ ] 2.5 Implement the user override with cross-session persistence and the unavailable-override fallback path
- [ ] 2.6 Implement the diagnostics report: discovered backends, active backend, selection reason, engine version, fallbacks this session
- [ ] 2.7 Add a test asserting a document exported on each registered backend agrees within parity tolerance and saves byte-identically

## 3. Rendering foundation

- [ ] 3.1 Create the window and event loop, and initialize the wgpu device and surface independently of the engine backend
- [ ] 3.2 Implement device-loss detection and resource recreation without losing the document
- [ ] 3.3 Implement the mesh renderer: vertex/index buffers, per-draw uniforms, depth, and dynamic buffer growth
- [ ] 3.4 Implement MatCap shading with a built-in material set and vertex-color modulation; verify no field math is present in any WGSL source
- [ ] 3.5 Implement the camera: orbit, pan, zoom, frame-all, frame-selection, and the empty-document default view
- [ ] 3.6 Implement the view presets with orthographic projection for the orthogonal views and framing preservation on switch
- [ ] 3.7 Implement the navigation gizmo with per-axis activation
- [ ] 3.8 Implement the ground grid and symmetry-plane overlays, excluded from export
- [ ] 3.9 Implement LOD selection over the brick cache mips with restoration on approach

## 4. MVVM skeleton

- [ ] 4.1 Define the Model interface in `clayspace-model` as a trait so ViewModels can be tested against a double
- [ ] 4.2 Define the command type and the single dispatch path; route every mutation through it
- [ ] 4.3 Implement the observable state mechanism with change signals that reading does not trigger
- [ ] 4.4 Implement the asynchronous command executor with progress reporting and stale-result discarding
- [ ] 4.5 Implement the composition root in `clayspace-app` and inject dependencies downward
- [ ] 4.6 Add the CI architecture check: dependency direction, no engine crates in `clayspace-view`, no `egui`/`wgpu`/`winit` in `clayspace-vm`, no `unsafe` outside the bridge
- [ ] 4.7 Write ViewModel tests that run headlessly against the Model double

## 5. Sculpting loop

- [ ] 5.1 Implement stroke capture: position, pressure, timing, on both pointer and tablet input
- [ ] 5.2 Implement the sculpt command path: capture → engine stroke resolution → edit → dirty bricks → re-mesh → upload
- [ ] 5.3 Implement incremental re-meshing: pass the dirty key set as the meshing subset, patch GPU buffer sub-ranges from the per-key ranges, discard stale results
- [ ] 5.3a Implement mesh buffer fragmentation tracking and background whole-surface compaction, scheduled off the interaction path
- [ ] 5.4 Implement the tool registry binding every tool label to its engine entry point, with per-representation availability and stated reasons
- [ ] 5.5 Implement per-tool settings persistence for intensity, size and flow
- [ ] 5.6 Implement the brush shaping controls: alpha curve, noise, edge falloff, accumulation mode, smoothing, mirroring
- [ ] 5.7 Implement symmetry about X, Y and Z with mirrored edits inside one undo group
- [ ] 5.8 Implement the brush cursor: projected radius, surface point, live size updates, off-surface state
- [ ] 5.9 Implement no-op detection using `change_count` deltas so dead edits produce no history entry
- [ ] 5.10 Implement the remaining tools: Puxar, Pinçar, Magnify, Raspar, Planar, Polir, Preencher, Nudge, Camada, Trim

## 6. Masks and armatures

- [ ] 6.1 Implement mask painting through the stroke engine, and pass the active mask to every verb
- [ ] 6.2 Implement mask invert, clear, expand, contract, smooth and bounded complement
- [ ] 6.3 Implement mask extrude with outward, inward and centred modes and a roundable rim
- [ ] 6.4 Verify masks survive a resolution change and that a fully masked region resists every tool
- [ ] 6.5 Implement armature authoring: add, move, resize, reparent, remove, skin thickness, and symmetric authoring
- [ ] 6.6 Verify armature persistence across save and reload

## 7. Scene, layers and history

- [ ] 7.1 Implement the scene tree bound to the engine node structure with expand, collapse and select
- [ ] 7.2 Implement the layer stack: create, rename, reorder, remove, intensity, visibility
- [ ] 7.3 Implement the three protection states with refusal-and-reason on protected edits, and ghost exclusion from picking
- [ ] 7.4 Implement selection through the engine's attributed raycast, synchronized across viewport, tree and stack
- [ ] 7.5 Implement layer transforms as single undo steps
- [ ] 7.6 Implement the field report and consolidation flow with the cost estimate shown before confirmation
- [ ] 7.7 Implement geometry statistics display, stating the resolution the counts describe
- [ ] 7.8 Implement mesh layers as carried content with sculpting tools disabled and the reason stated
- [ ] 7.9 Implement undo and redo over the engine's undo vocabulary, with stroke and drag coalescing
- [ ] 7.10 Implement undo groups for compound operations
- [ ] 7.11 Implement the history panel with named entries, current position and jump-to-entry
- [ ] 7.12 Implement the bounded history depth and the redo-branch discard with a visible outcome
- [ ] 7.13 Verify camera, view, material, layout and selection changes create no history entries

## 8. Document lifecycle

- [ ] 8.1 Implement open and save through the engine's document I/O, with the newer-version refusal path
- [ ] 8.2 Verify cross-platform byte-identical documents in CI
- [ ] 8.3 Implement mesh import for OBJ, PLY, FBX and GLB with the engine's guardrails and the raised-ceiling path
- [ ] 8.4 Implement export with mesher choice, resolution, decimation and attribute-support warnings; default to the watertight mesher
- [ ] 8.5 Implement autosave recovery state, crash detection and the recovery offer
- [ ] 8.6 Implement the unsaved-changes decision on close and on quit
- [ ] 8.7 Implement the recent documents list with missing-file pruning
- [ ] 8.8 Implement the document working unit and presentation-only unit switching

## 9. Interface shell and design system

- [ ] 9.1 Implement the design tokens: palette, spacing scale, control sizing, radii; add a check that no literal colors exist in components
- [ ] 9.2 Implement typography: humanist sans for labels, monospaced for numeric readouts with fixed digit positions
- [ ] 9.3 Implement the icon set at one stroke weight and optical size
- [ ] 9.4 Implement the region layout: menu bar, tool rail, tool options bar, left region, viewport, right region, brush shelf, status area
- [ ] 9.5 Implement panel resize, collapse, layout persistence and reset-to-default
- [ ] 9.6 Implement the left region: scene tree, layer stack, sculpting settings (symmetry, resolution, smoothing, save preset)
- [ ] 9.7 Implement the right region: material inspector, geometry statistics, resolution controls, brush controls
- [ ] 9.8 Implement the brush shelf with skeuomorphic sphere previews and the accent on the active brush only
- [ ] 9.9 Implement the menu bar with shared command dispatch, shortcut display and matching disabled conditions
- [ ] 9.10 Implement remappable shortcuts with conflict reporting
- [ ] 9.11 Implement the status area: document name and modified state, unit, memory against budget with an early-warning state, active backend
- [ ] 9.12 Implement budget-exhaustion handling with the shortfall shown and the document left intact
- [ ] 9.13 Implement error presentation near the failing action with engine detail available in diagnostics
- [ ] 9.14 Externalize all user-facing strings; ship pt-BR; implement locale fallback and long-label layout tolerance
- [ ] 9.15 Verify the contrast floors and that state is never conveyed by color alone
- [ ] 9.16 Verify the style budget: panels, buttons, sliders and menus carry no skeuomorphic treatment

## 10. Performance and packaging

- [ ] 10.1 Define the reference document and reference machine configuration per platform
- [ ] 10.2 Implement the benchmark harness for dab latency, frame time, edit locality, startup and memory
- [ ] 10.3 Measure and record baseline figures; wire the budgets as a CI gate reporting before and after
- [ ] 10.4 Implement the interface-thread blocking instrumentation with a 16 ms threshold
- [ ] 10.5 Verify edit cost does not scale with scene size using the ten-times-larger comparison scene
- [ ] 10.6 Verify memory returns to baseline across repeated open, sculpt, close cycles
- [ ] 10.7 Add build features for the backends, with CPU always compiled in and clear failures for unavailable toolkits
- [ ] 10.8 Set up the CI matrix: macOS CPU-only, macOS Metal, Linux CPU-only, Linux accelerated
- [ ] 10.9 Add the OpenSpec strict validation job
- [ ] 10.10 Add formatting, lint and dependency-audit gates
- [ ] 10.11 Embed the application version and pinned engine revision, and surface both in diagnostics
- [ ] 10.12 Produce the macOS bundle and the Linux distributable, self-contained with respect to the engine
- [ ] 10.13 Generate the attribution manifest and surface it in the application

## 11. Close-out

- [ ] 11.1 Resolve the open questions in `design.md`: the Dinâmica panel scope, localization scope, and the default document representation
- [x] 11.2 Write the README covering prerequisites, submodule initialization, build, and the backend matrix
- [ ] 11.3 Run `openspec validate --all --strict` and the full test suite; archive the change
