# Tasks

## 1. Move the pin

- [x] 1.1 Point the submodule at v0.84.0 and move `EXPECTED_ABI`, which
      `version_is_the_pinned_engine` holds to the submodule. It moves by hand
      and is deliberately not derived from the linked engine, which would make
      the check assert that a number equals itself
- [x] 1.2 Build the whole workspace against the new engine before changing a
      line of it, so that what the upgrade *forces* is separable from what it
      *enables* — it forces nothing: 29 entry points added, none removed, no
      signature changed, and the one descriptor that grew did so behind the
      `struct_size` this workspace writes from `size_of`
- [x] 1.3 Count the added and removed entry points from the two tags' own
      headers rather than from the release notes; verify 29 added, 0 removed

## 2. Decide what to write, since the container moved

- [x] 2.1 Establish whether the upgrade notes' advice — write at minor 16 if you
      exchange documents with an older build — is reachable. It is not, for the
      third release running: the `minor` parameter is on the C++
      `serialize_document`, not on `save_clayspace`, and not on
      `clay_document_save`, which takes a path and nothing else
- [x] 2.2 Move `Document::FORMAT` to minor 17 and rewrite the reasoning beside
      it for this release, naming what the minor buys (a payload written once
      per document rather than once per holder) and what it costs (a document
      refused by a build older than v0.84.0); verify
      `the_constant_is_the_minor_the_pinned_engine_writes` and
      `a_file_this_build_writes_says_the_minor_this_build_claims`

## 3. Triage every failure against the release notes

- [x] 3.1 Run the suite and triage rather than assume staleness. Two failures,
      both `sdf_brushes`, both Suavizar on a pristine sphere
- [x] 3.2 Measure the change across the pin instead of adjusting the threshold
      to fit it: a pick against the resting unit sphere reads 1.006707 on
      v0.78.0 and 1.000296 on v0.84.0, because the brick cache is walked
      analytically now
- [x] 3.2a **Then measure it with something that is not a pick**, because the
      first attempt read a surface with the instrument that changed and
      concluded the verb was stronger. `clay_eval_points` over 2,197 points
      about the stroke: the field moves **0.0400000 on both pins** for one
      stroke and **0.1198471 on both** for four dabs, to the last digit. The
      brush is bit-identical and the pick is the whole difference — a stroke
      that moves the field by 0.04 read as `1.0350003` twice on v0.78.0
- [x] 3.3 Give the four region-sampling verbs something to sample, using the
      engine adapter's own grouping — "Suavizar, Relaxar, Planar and Polir do
      not stamp: they sample a region" — keeping the 1e-3 threshold rather than
      lowering it; verify `every_surface_brush_moves_the_surface` and
      `every_surface_brush_mirrors_when_it_is_asked_to`

## 4. Confirm what the release claims to give back

- [x] 4.1 Re-measure `object.drag_frame_intersect` against its subtracting
      control on the same fixture, machine, backend, viewport and scenes — four
      runs of `just bench-only object` at 0.14 load per core, which is quieter
      than the campaign it answers. Intersect 66.84 → **64.81 ms** with the
      control flat at 25.46: **21% of the regression back, still 1.130x
      v0.73.0**
- [x] 4.2 Record it as *partly* recovered rather than as closed, with the
      reason the two answers differ — upstream fixed the layer extent bound
      query, whose own published figures are 0.0669 ms a frame to 0.0003, and a
      fix worth 0.067 ms cannot account for 9.49. Both statements hold and the
      report carries both
- [x] 4.3 Confirm the claim that *is* visible here: `object.pick.ms` 0.113 →
      0.055, **0.49x** against a claimed 0.51–0.54x, because this application
      calls `clay_raycast_attributed` directly

## 5. Hold it

- [x] 5.1 The whole suite against the new pin — 2235 passed, 0 failed, 2
      ignored across the workspace, and 894 across `claycore` and
      `clayspace-engine` again after the `resume_stats` reader was added
- [x] 5.2 `just check` — formatting, layering (18 edges, unsafe confined),
      clippy at `-D warnings` on `--all-targets --release`, the suite, the
      specification and the packaging scripts
- [x] 5.3 The specification on **both** validators, because CI installs
      `@fission-ai/openspec@latest` while `just spec` runs whatever is installed
      locally, and those were not the same tool: local **1.11.0**, CI
      **1.12.0**. 37/37 on both. They agree today by luck rather than by
      design — a gate whose enforcing version floats can change under a tree
      nobody has touched, and a result reported from the local one is not the
      result CI will produce

## 6. Say it

- [x] 6.1 `README.md` — the pin, the container minor, the diagnostics sample,
      and the two figures this release moved: a pick at 0.49x, and a resting
      unit sphere reading 3e-4 from ideal where it read 6.7e-3, which is why two
      brush tests were corrected rather than re-thresholded
- [x] 6.2 `docs/roadmap.md` — the twenty-nine entry points this application
      calls none of, why that is a line rather than a backlog, and the two worth
      real work with the order to take them in: the prefix cache first, because
      the instrument that would show it already exists here

## 7. What this pin found that was not the pin's

- [x] 7.1 `Document::resume_stats` — a safe reader for
      `clay_document_resume_stats`, which had none. Added to answer whether a
      transform-driven refill resumes; it does not, and that is by design
      upstream, because a seed is only valid across appends
- [x] 7.1a Cover it. The wrapper shipped with **no gate over it at all** — its
      only callers were the two probes kept outside the suite — which is this
      change's own instance of the thing this session spent a day naming
      elsewhere. `the_resume_counters_only_ever_climb` asserts the counters are
      cumulative, that a refill after an edit moves one of them, and *which*
      one: an append resumes every brick (0 → 64, refilled unmoved at 64),
      where a gizmo drag resumes none. Its first two drafts passed vacuously
      and its own anti-vacuity guard said so both times
- [x] 7.2 `place_layer`'s refusal comment stops asserting a cause this side
      never measured. Which limit bound, and what the region and budget were,
      are facts the engine has and this side does not; the sculptor sees the
      engine's own sentence, and the comment now says so rather than guessing
      at a scale
- [x] 7.3 `benchmarks/probes/` — the two fixtures that measured
      [#471](https://github.com/CyberdyneCorp/ClayCore/issues/471), kept out of
      the suite deliberately: they are instruments rather than gates, and the
      10x intersect case alone is minutes of wall clock

## 8. The one target the suite does not reach

- [x] 8.1 `agent_end_to_end` is behind the `agent-e2e` feature and CI **builds
      and lints it without running it**, under a comment claiming that this is
      why "it cannot rot unnoticed". It has: the target fails, and failed on
      main before this pin moved. A compile is not a run, and a comment that
      says otherwise turns an unknown into a false known
- [x] 8.2 Establish whether the pin changed it, on a machine quiet enough to
      answer. On a loaded box (load 52 of 24 cores, a peer session's gate run)
      both cases failed at setup — "the application published no address within
      60s". Settled to load ~10 and re-run: **1 passed, 1 failed, identical to
      main at v0.78.0.** The application starts; the pin is clean
- [x] 8.3 The surviving failure is the pre-existing one — an agent-driven
      stroke moving no pixel, `past_the_floor: 0` against a measured floor of 0
      — and it belongs to the change that introduced the target, not to this pin
- [ ] 8.4 **A 60-second startup deadline a shared box can blow is a flake
      generator.** Measured comfortable at load ~10 and blown at load 52, same
      commit, same binary. Not this change's to fix, and raised here so it is
      not rediscovered

## 9. The corrected brush test, checked against its own justification

- [x] 9.1 Apply the question this change's counterpart asked of its own modified
      tests — *would this file still fail if the pin were reverted, and is the
      thing that fails a line that existed before?* — to the one substantive
      code change here. Reverted the submodule to v0.78.0 and ran the corrected
      `sdf_brushes`: **it fails there.** So it is documentation of the new
      behaviour rather than a regression test that would have caught the old
- [x] 9.2 Record what that costs the justification. Task 3.2 says the fixture
      was measuring the pick's own error as clay, which is true and was measured
      — but it implies the corrected fixture is pin-neutral, and it is not. Both
      the old fixture and the new one distinguish the pins, in opposite
      directions
- [x] 9.3 **And it surfaced a behaviour change — which turned out to be the
      instrument again.** On
      v0.78.0 a Suavizar *stroke* over a bump leaves the surface **bit
      identical** — `1.0350003 from 1.0350003` — while four separate dabs at
      the same settings moved it 0.0084 on the same pin. On v0.84.0 the stroke
      moves it past the 1e-3 the file asks of every brush. **Settled by task
      3.2a: there is no stroke-path defect.** The field moves identically on
      both pins, so `clay_item_volume_relax_from` — which is what the non-live
      Suavizar calls, and not `clay_sdf_smooth_*` — behaves the same. The
      corrected fixture is still pin-sensitive, and now for a reason that is
      understood rather than open: it reads the surface with a pick
