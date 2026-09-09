# Tasks

## 1. The seam that needs no new dependency

- [x] 1.1 Bind `clay_mesh_save_handoff`, `clay_mesh_save_handoff_memory` and
      `clay_mesh_handoff_material_mix` in `claycore`, carrying
      `CLAY_HANDOFF_VERSION_MAJOR`/`MINOR` from the header rather than
      restating them; verify a written file is accepted by the pinned
      CyberRemesher CLI, since our own parser agreeing with us proves nothing
- [ ] 1.2 A **Ficheiro → Exportar para retopologia…** command writing the PLY
      profile, with a mask offered for `material_mix` and a null mask writing
      zeros, which is the honest answer for a document that never expressed one
- [ ] 1.3 A guard that the handoff writer is used and `clay_mesh_save` is not:
      their reader rejects any arity but triangles, so a mesh carrying quads
      exported the general way is exactly the file the pipeline refuses; verify
      by exporting a quad-carrying mesh both ways and asserting which is
      accepted

## 2. Vendor and bind, at v0.7.0

- [x] 2.1 Submodule at `vendor/CyberRemesherAndUV` pinned to **v0.8.0**, with
      the superproject-revision check `claycore-sys` already performs. Not
      v0.7.0: it carries a PLY header that sizes an allocation from an
      attacker-controlled count, and this application imports meshes
- [x] 2.2 `cyberremesh-sys`: CMake through the `cpu-headless` preset, bindgen,
      link flags, rerun directives; a missing submodule or an old CMake
      reported before the linker speaks
- [x] 2.3 `cyberremesh`: the safe wrapper. Owning types that free what the C ABI
      returns — meshes, images, bundle results, seam sets, layouts — so a
      refused call cannot leak one
- [x] 2.4 `cyber_version` checked against a declared constant; verify the test
      fails when the submodule moves and the constant does not. **The pin is the
      commit** — `SOVERSION` is the project major, still 0, so
      `libcyber_capi.so.0` names two different releases and a mismatched library
      loads silently. Assert an ABI-level number the day the engine grows one
- [ ] 2.7 **Half done, and the other half is not possible at this ABI.**
      Configure with `CYBER_REQUIRE_QUADCOVER=ON` so a missing dependency
      fails the build rather than falling back: **done**, and it is a hard
      `FATAL_ERROR` at `cmake/QuadCoverSolver.cmake:105`, so a build that
      cannot have the in-process field does not produce a library that quietly
      quadrangulates differently — it fails to configure. Confirmed on the
      compile line, which carries `-D CYBER_HAVE_QUADCOVER`.

      Reading the solver string at startup: **not done and not reachable.**
      `cyber_capi.h` exposes no solver-name entry point — `grep -n solver`
      over the header returns prose and nothing else. A `Solver` enum was
      written against this task and was dead on arrival: nothing could
      construct it, while its doc asserted the solver is "readable at all".
      Removed, with the reasoning kept in `version.rs` so it is not rewritten
      by the next reader who has the same idea.

      **The risk that half was guarding is therefore still open, and is worth
      naming rather than closing with the task:** a *different*
      `libcyber_capi.so.0` being loaded than the one we built, which their
      soname permits because it names every 0.x release. The build-time flag
      cannot see that; only a runtime read could. So this stays unticked until
      the engine exposes the solver, and the ask is filed with them rather
      than worked around here — a string the CLI prints is the CLI's own
      report, not the library we link, so reading it would measure the wrong
      artifact
- [x] 2.8 A `CyberMesh` is built, used and dropped per operation, and no id is
      cached across one; the engine reassigns element ids on most retopology
      calls and **all** of them on subdivide
- [x] 2.5 `tools/check_layering.py`: both crates on the `unsafe` allowlist, and
      forbidden to `clayspace-view`, `clayspace-vm` and `clayspace-mcp` — the
      same edges `claycore` already has, for the same reasons
- [ ] 2.6 Record the added configure-and-build time and the artifact size
      growth, both measured rather than estimated

## 3. Hold the sculpting budget, before anything is built on it

- [x] 3.1 `cyber_set_max_worker_threads` at startup, never uncapped; verify a
      capped run is byte-identical to an uncapped one, which their own test
      pins and ours should not have to trust
- [x] 3.2 The CPU backend asserted at startup — `cyber_active_backend` — so a
      build that quietly acquired CUDA fails a test rather than a stroke
- [ ] 3.3 Record `dab.*`, `brush.*`, `locality.*`, `tape.*`, `startup.*` and
      `memory.*` **before the library is linked**, on a quiet machine, load
      stated. This is the half that cannot be retaken
- [ ] 3.4 Record them again after, same machine, one sitting; compare as ratios
- [ ] 3.5 A `retopo.*` benchmark group, so the new operations have a history
      from their first day

## 4. Quads

- [x] 4.1 `RetopoModel` in the domain: quad method, target count, the result as
      a new subtool. No engine types
- [x] 4.2 The adapter over `cyber_remesh`, offered on mesh subtools only
- [x] 4.3 Through `jobs`, with progress and supersession; refused while a
      gesture is open
- [x] 4.4 One undo entry for the placed result; verify the history depth moves
      by one and undo removes the subtool
- [x] 4.5 Only the quad methods the pinned build carries appear in the
      interface; verify against the engine rather than a hard-coded list

## 5. Conform

- [x] 5.1 `cyber_conform` behind a `ConformModel`, reporting maximum and RMS
      deviation and the count past a caller-set threshold
- [x] 5.2 Those figures reach the sculptor rather than being dropped into a
      success; verify the reported count is non-zero for a source moved further
      than the threshold

## 6. UV

- [x] 6.1 `cyber_uv_atlas_cancellable` behind a `UvModel`, reporting chart
      count, conformal distortion, flips and packing efficiency
- [ ] 6.2 `cyber_uv_unwrap_seams` along a seam set, with an empty set meaning
      *do not cut*; verify the two requests are distinguished
- [ ] 6.3 The seam-path tool — waypoints, re-route on edit, commit into a seam
      set, the commit's edge list as the undo record
- [ ] 6.4 The UV layout drawn, and its distortion readable

## 7. Baking from the field

- [x] 7.1 A `CyberFieldEvaluator` filled from `clay_eval_points`,
      `clay_eval_gradients` and `clay_measure_points`
- [x] 7.2 **The occlusion inversion**, held by a test: theirs is openness where
      1 is open, ours is occlusion where 1 is enclosed. Verify the test fails
      when the `1.0 -` is removed, since an inverted AO map looks plausible
- [x] 7.3 **Curvature left to their default**, which derives it from the
      gradient; verify `CLAY_MEASURE_CURVATURE` is not wired to it, the two
      being a saturated masking value and a signed quantity in `1/length`
- [x] 7.4 Normal, AO, curvature and cavity baked with no target mesh; every
      other map still takes the raycast path
- [ ] 7.5 The maps written, and the export presets offered as the engine
      resolves them rather than as a list restated here

## 8. Say it

- [ ] 8.1 `README.md` and `docs/features.md`: the pipeline, what each engine
      owns, and the seam between them
- [ ] 8.2 The three traps written where the code is, not only here
- [ ] 8.3 `docs/architecture.md`: a second vendored engine, the layering edges
      it added, and why the CPU preset

## 9. What is deliberately not in this change

- [ ] 9.1 `--quality best` is offered rather than defaulted: it costs a second
      full field solve for a small gain, and a sculptor should choose to wait
      rather than be made to
- [ ] 9.2 The manual retopology toolkit — ~40 `cyber_retopo_*` calls, soft
      selection, stroke interpretation — is a second interaction model and a
      change of its own
- [ ] 9.3 No routing rule of our own between quad solvers. Their README records
      that choosing per input "is still open work"; we take their default

## 10. What we owe the engine, being the host it never had

The retopology engine built its sculpt-handoff reader with no producer and its
field evaluator with no field. We are the first of either.

- [x] 10.1 Report whether a real producer's handoff ever carries non-triangles.
      Measured answer so far: **no** — ClayCore's handoff writer triangulates
      before the bytes exist, so their triangles-only check costs us nothing and
      SHOULD NOT be relaxed on our account
- [ ] 10.2 Check the field evaluator's hemisphere convention against a real SDF
      **at grazing angles**, which is where they had to assume
- [ ] 10.4 Set our own resource ceiling on any import routed through the
      retopology engine — it has **none**, and its `max_vertices` parameters are
      output buffer sizes rather than limits. Its structural bound (declared
      count against file size, by division) is a hostility bound, not a resource
      one: it refuses a small file making a large claim and admits a legitimate
      200M-vertex mesh. Do **not** write the assertion that our ceiling sits
      under the engine's, as the ClayCore path has; there is no engine number
      there to sit under
- [ ] 10.3 Report whether `cyber_set_max_worker_threads` actually holds sculpting
      latency flat under load. They can only simulate this; we have `dab.*`,
      `brush.*`, `locality.*` and `tape.*` and a before-and-after protocol, so
      they get a measured answer instead
