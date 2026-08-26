## 1. The benchmark's skeleton

- [x] 1.1 Split `crates/clayspace-app/src/bin/bench.rs` into `src/bin/bench/main.rs`, `figures.rs`, `report.rs`, `json.rs`, `compare.rs` and `groups/`, moving the existing six measurements across unchanged, and confirm `just bench` still prints the same twenty figures
- [x] 1.2 Introduce the figure record: `Repeatable` (12 samples, median and p95, tolerance 1.5) and `OneShot` (3 samples with a rebuild between, median, tolerance 2.0), declared in one table in `figures.rs`
- [x] 1.3 Introduce the skip: a measurement returns figures or a fixed `Skipped { reason }`, and the report prints a skipped section
- [x] 1.4 Convert the six existing measurements' `let ... else { return; }` bails into skips with reasons — no headless GPU, backends undiscoverable, scene would not build
- [x] 1.5 Time each group and the whole run, and print the durations under the table
- [x] 1.6 Add `--only <prefix>`, filtering the figures measured and reported; refuse `--only` together with `--json` with a message saying why

## 2. The baseline file grows a shape

- [x] 2.1 Replace `Conditions::scene` with `scenes`, a member-to-revision map, and write it into the JSON
- [x] 2.2 Replace the `str::find` baseline reader with a parser for this file's shape, in `json.rs`, with unit tests over a written-then-read round trip
- [x] 2.3 Record skips in the JSON alongside figures
- [x] 2.4 Make `compare` refuse on a `scenes` mismatch and name the member that differs
- [x] 2.5 Make `compare` classify each baseline figure as present, skipped or missing; report all three; fail the gate on missing
- [x] 2.6 Test the comparison: a regression fails, a skip does not, a missing figure does, an unlike suite refuses, a baseline without `scenes` refuses

## 3. The reference suite

- [x] 3.1 Add a voxel reference scene — a worked grid at the cell size the application uses, deterministic, revisioned as `voxel-reference-r1`
- [x] 3.2 Add a mesh reference scene, at a triangle count the mesh brushes are actually used at, revisioned as `mesh-reference-r1`
- [x] 3.3 Give each suite member a probe brush and a probe stroke, as `Scene::probe_brush` and `Scene::probe_point` do for the SDF scene, so an edit lands on the surface rather than under it
- [x] 3.4 Test that each member builds to a stated surface size within a tolerance, so a member that silently stopped building the same thing is caught rather than measured

## 4. Every brush

- [x] 4.1 Add the `brush` group, iterating `Representation::ALL` × `ToolKind::for_representation`, applying each tool with the gesture it takes and timing `apply_stroke` plus `sync`
- [x] 4.2 Drive the region-based four (Suavizar, Relaxar, Planar, Polir) as one whole-gesture application rather than per segment, since they do not decompose
- [x] 4.3 Drive the path-driven three (Mover, Puxar, Nudge) with a segment that carries where it started from
- [x] 4.4 Record a skip with a reason for any derived pair the harness cannot drive, and check the report names them
- [x] 4.5 Check the brush figures against `just segments`' printed per-segment costs, and reconcile any figure that disagrees by more than noise before recording anything

## 5. Layer operations, deformers and rigging

- [x] 5.1 Add `LayerOperation::ALL`, built from an exhaustive `match` so a new variant fails to compile
- [x] 5.2 Add the `op` group: taper, twist, lattice drag, close holes, fill voids, refine region, each on the representation that carries it
- [x] 5.3 Add the `deform` group: the deform panel's operations applied to a layer as `RunDeform` applies them — folded into `op`, since `DeformSettings::operation()` *is* a `LayerOperation` and a separate group would report the same engine work twice under two names
- [x] 5.4 Add the `armature` group: authoring the reference rig and skinning it, reusing the rig `visual_armature.rs` already builds
- [x] 5.5 Add the `curve` and `lattice` figures for the authoring operations that reach the document — the cage is `op.mesh.lattice`, the curve is `authoring.curve`

## 6. Conversions, bakes and export

- [x] 6.1 Add the `convert` group: each direction across SDF, voxel and mesh, at the suite's cell size
- [x] 6.2 Add consolidation, on a layer with enough history for it to mean something
- [x] 6.3 Add export of the reference scene, timing the write to a scratch path and deleting it
- [x] 6.4 Add voxel repair — the report, close holes and fill voids — on a grid built to have something to repair — the report is `bake.repair_report`; the two repairs are `op.voxel.*`, and both now run on a new suite member, `voxel-pocked`, which has a sealed pocket and a bored channel in it

## 7. Masks and history

- [x] 7.1 Add the `mask` group: painting a mask, and the same stroke gated and ungated as a ratio
- [x] 7.2 Add the `history` group: undo and redo, absolute and as a ratio against the edit they reverse
- [x] 7.3 Check the undo ratio against what `undo_cost.rs` measures, and reconcile before recording

## 8. Recording and documentation

- [x] 8.1 Run the full suite on Linux, check the twenty carried-over figures still match the old baseline within noise, and re-record `benchmarks/baseline-linux-x86_64.json` in its own commit with the reason in the message — the carried-over twenty all matched inside ±10 % with the counts identical, and the gate was then run against its own recording until it passed on an unchanged tree, which took two corrections: a gesture is reported by its mean rather than its median, and every run warms the machine first
- [x] 8.2 Mark `benchmarks/baseline-macos-aarch64.json` stale, so a macOS comparison refuses rather than reporting the difference between two suites as regressions
- [ ] 8.3 Re-record the macOS baseline on a macOS machine, in its own commit
- [x] 8.4 State the full run's wall clock in `justfile`'s bench recipes, and add a recipe for a filtered run
- [x] 8.5 Update `docs/architecture.md` and `README.md` where they describe what the gate covers
- [x] 8.6 Point `stroke_budget.rs`'s cross-version table and `undo_cost.rs`'s narrative at the baseline as the record, keeping their explanations
- [x] 8.7 Run `just check`, and the `cognitive-complexity` skill over `src/bin/bench/`, keeping every function inside the frontend band — the skill measures C/C++, Python, Go and TypeScript and has no Rust analyser, so the band is held by construction instead: every measurement is a `build → arrange → time → record` sequence whose branches are `?` on a `Result<_, Skip>`, the longest measurement is 46 lines, and the longest function of any kind is `main` at 58, which is a linear sequence of flags, groups and gates rather than a nest of them
