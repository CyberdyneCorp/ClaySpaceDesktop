# Tasks

## 1. The bookkeeping, with no clock in it

- [x] 1.1 Add `clayspace-model/src/profile.rs` with `Phase`, `Work`,
      `Samples` and `StrokeProfile` — durations taken from the caller, no
      `Instant`, no engine types, as `instrument.rs` is — and export it from
      `lib.rs`; verify `cargo test -p clayspace-model` passes with the module
      compiled in
- [x] 1.2 `Samples::record` keeps a ring of the last 4096 durations plus an
      unbounded `seen` and an unbounded `worst`; verify a test recording 5000
      samples reports `seen` 5000, `retained` 4096, and a `worst` taken from a
      sample that has since left the ring
- [x] 1.3 `Samples` reports median and p95 over what is retained, and reports a
      phase with no samples as having none rather than as costing zero; verify
      `a_phase_that_never_ran_reports_no_samples_not_a_zero`
- [x] 1.4 `StrokeProfile::record` folds a sample under its tool, and
      `StrokeProfile::across_tools` computes the aggregate on read rather than
      keeping it twice; verify a test asserting the aggregate of two tools
      equals the phases recorded, and that no field holds it

## 2. Measure the engine's half

- [x] 2.1 `SharedDocument` holds an `Rc<RefCell<StrokeProfile>>` and its
      `apply_stroke` times the engine call, recording `Phase::EngineEdit` with
      `EditOutcome::dirty_bricks` as the workload; verify a test applying three
      strokes through `SharedDocument` finds three `EngineEdit` samples
- [x] 2.2 A refused stroke records nothing — an error is not a measurement of
      the engine doing the work; verify
      `a_refused_stroke_leaves_the_profile_untouched`
- [x] 2.3 `SharedDocument::profile` hands the profile out for reading; verify
      it compiles against `dab_profile.rs` reading the same numbers it
      currently computes by hand

## 3. Stop discarding the other half

- [x] 3.1 `sync_geometry_now` folds the `SyncCost` it already receives into the
      profile as four samples — `EngineMesh`, `Read`, `Split`, `Upload` — with
      keys and triangles as the workload, instead of matching `Ok(_)` and
      dropping it; verify a headless test that dabs once and finds all five
      phases populated
- [x] 3.2 A sync that re-meshed nothing records nothing; verify
      `a_sync_with_no_dirty_keys_records_no_samples`

## 4. The report says which side

- [x] 4.1 `Diagnostics` gains a `stroke: Option<StrokeDiagnostics>` carrying
      per-phase count, median and worst, and which side of the boundary each
      phase is; verify the model's own `to_report` test shows the five rows
- [x] 4.2 `to_report` renders the stroke section with the engine's phases named
      as the engine's and ours as ours, and renders "no samples" where a phase
      never ran; verify both cases in `diagnostics.rs`'s tests
- [x] 4.3 Plumb `BackendPolicy::refill_cost_per_brick` through `ClayDocument`
      into `Diagnostics`, reporting an unmeasured backend as unmeasured rather
      than as zero; verify `an_unmeasured_backend_is_not_reported_as_free`
- [x] 4.4 Fill `Diagnostics::stroke` and the refill line at the composition
      root, where the renderer and the stalls are already filled; verify the
      pasted report from a run of the app carries them

## 5. Show it

- [x] 5.1 A **Esforço da pincelada** section in the diagnostics window, one row
      per phase, engine rows grouped above ours; verify against
      `target/visual/` that the window still fits and reads
- [x] 5.2 Strings for the section, the phase names and the refill line in all
      three locales; verify `cargo test -p clayspace-view` string-table
      completeness test passes

## 6. The file

- [x] 6.1 A `Json` writer in `clayspace-app` owning nesting, commas and string
      escaping; verify unit tests for escaping a quote, a backslash and a
      control character, and for balanced containers
- [x] 6.2 `profile_file::render` — conditions, build profile,
      `timings_comparable`, document shape, per-phase distributions per tool
      and across tools, stalls, fallbacks, GPU passes, refill costs, memory;
      verify a golden test over a fixture profile asserting every declared key
      is present
- [x] 6.3 A phase, backend or adapter figure that was not measured is written
      as unmeasured, never as zero; verify
      `nothing_unmeasured_is_written_as_a_zero`
- [x] 6.4 Subtools are written as representation and index, and no document
      path or user-chosen name enters the profile or the writer; verify
      `a_named_subtool_does_not_reach_the_file`, asserting over the whole
      rendered string
- [x] 6.5 The top-level `build` and `timings_comparable` fields are stamped
      from `cfg!(debug_assertions)`; verify a test asserting the pair agree and
      that a debug build declares its timings not comparable

## 7. Reach it

- [x] 7.1 `Command::ExportProfile` with a label, dispatched like the other
      file-writing commands; verify the command's label appears in the stall
      list naming if it ever stalls
- [x] 7.2 **Ajuda → Exportar perfil…** beside Diagnostics, opening a save
      dialog defaulting to `perfil.json`; the menu entry and the window button
      are verified in `target/visual/64-diagnostics.png`, and the dialog's own
      parameters — `profile_file::FILE_NAME` and `EXTENSIONS` — by
      `the_save_dialog_offers_what_its_default_name_already_is`
- [x] 7.2a The diagnostics window scrolls its sections and pins its buttons —
      the report had already outgrown an 800-pixel screen before this change
      added a section, putting the copy button past the window's own edge;
      verify in `target/visual/64-diagnostics.png`
- [x] 7.3 On a debug build the export states, before writing, that the timings
      in the file are not comparable; the file's own two markers are verified
      by `the_file_declares_whether_its_timings_mean_anything`, the decision by
      `a_debug_build_asks_first_and_a_release_build_does_not`, and the sentence
      itself in all three locales by
      `the_debug_profile_warning_says_what_it_is_for_in_every_language`
- [x] 7.4 A path that cannot be written reports and leaves no partial file;
      verify `a_failed_export_leaves_nothing_behind` writing to a directory
      that does not exist
- [x] 7.5 Exporting from a session that has applied no stroke still writes a
      file, with zero-sample phases; verify
      `an_unworked_session_still_exports`

## 8. Hold it

- [x] 8.1 `cargo test -p clayspace-app --test sculpt_latency --release` still
      passes — the two added `Instant::now` pairs must not move the dab budget;
      verify median and p95 against `benchmarks/baseline-linux-x86_64.json`
- [x] 8.2 `just bench-compare` shows no regression on `dab.*` and `brush.*`;
      verify against the committed baseline on a quiet machine
- [x] 8.3 `just check` — formatting, layering, clippy, the suite, the
      specification and the packaging scripts

## 9. Say it

- [x] 9.1 `README.md` — *Acceleration and diagnostics* gains the stroke
      breakdown and the export, with the debug-build caveat stated where the
      export is described
- [x] 9.2 `docs/features.md` — *Diagnostics* gains the same, and a *Profile
      export* subsection describing what the file carries and what it
      deliberately does not
- [x] 9.3 `justfile` — a `profile` recipe running the release build so the
      documented way to produce a file for upstream is the trustworthy one

## 10. Do not become what we are measuring

- [x] 10.1 `profile_overhead.rs` measures both costs apart: recording one
      phase, and summarising a worked session; verify it prints both and
      asserts recording stays under 5 µs
- [x] 10.2 `Samples::summary` answers median, p95 and worst from one sort
      rather than three; verify `one_sort_answers_the_same_as_three_questions`
- [x] 10.3 `SharedDocument::with_profile` reads the profile in place, so the
      report does not clone every retained window to summarise it; verify the
      borrowed figure in `profile_overhead.rs` against the cloned one
- [x] 10.4 `App::diagnostics` takes a `StrokeSection`, and summarises only for
      the open window, the export, and a request that asked; verify an idle
      frame with the window closed carries no stroke section

## 11. An agent reads it too

- [x] 11.1 `StateQuery` and `StateReport` gain a `strokes` section, and
      `from_sections` accepts and names it; verify the refusal lists it
- [x] 11.2 `report::stroke_state` renders the per-phase split for the wire,
      marking every figure as live-session; verify
      `every_figure_says_it_came_from_a_live_session`
- [x] 11.3 A phase that never ran is absent rather than zero on the wire;
      verify `a_phase_that_never_ran_carries_no_figure_rather_than_a_zero`
- [x] 11.4 `Command::ExportProfile` is placed in the catalogue as not offered —
      it opens a file panel — with the read named as what an agent wants
      instead; verify `clayspace-mcp` compiles, which the exhaustive `home_of`
      match is what enforces
- [x] 11.5 The `state` tool's description and section enum name `strokes`;
      verify by reading the tool descriptor

## 12. The dialogs, without a hand on the mouse

- [x] 12.1 `profile_file::Ask` and `ask_before_writing` hold the decision the
      warning carries, derived from `timings_comparable` rather than from a
      second `cfg!`; verify
      `the_question_asked_and_the_claim_written_cannot_disagree`
- [x] 12.2 `profile_file::FILE_NAME` and `EXTENSIONS` hold what the save dialog
      is opened with, so a default name that the offered filter would reject
      fails a test rather than a person; verify
      `the_save_dialog_offers_what_its_default_name_already_is`
- [x] 12.3 The warning moves into `Strings` in all three locales, so the
      completeness and divergence tests cover it; verify
      `the_debug_profile_warning_says_what_it_is_for_in_every_language`, which
      was confirmed to fail against a translation that keeps the tone and drops
      the fact that the timings do not travel
- [x] 12.4 No toggle for the recording. `profile_overhead.rs` measures it at 20
      ns against a two-millisecond dab; the 0.9 ms was the reporting, and task
      10 fixed that where it was
