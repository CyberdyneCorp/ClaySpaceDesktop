# Tasks

## 1. The performance gate

- [x] 1.1 A skip excuses a figure only when it is the machine's inability or the baseline gave the same reason (#32)
- [x] 1.2 `shell: bash` so `pipefail` stops `tee` discarding the exit status (#32)
- [x] 1.3 Tell a signal apart from a verdict, and name the issue the quarantine expires with (#32)
- [x] 1.4 A dispatchable job that records a baseline on the runner, since the Mac nobody had is the one CI runs on (#32)
- [ ] 1.5 Commit a macOS baseline recorded by that job, and make a refusal to compare a hard failure

## 2. Visual verification

- [x] 2.1 Restore the stroke `the_smoothing_tools_smooth_rather_than_crumble` stopped applying, and assert it moved something (#39)
- [x] 2.2 Count past `RENDER_NOISE` rather than byte-exact wherever a floor sat under the measured one (#39)
- [x] 2.3 Assert the occlusion count that was computed and printed (#39)
- [x] 2.4 Read a block and establish it is on the subject, rather than one centre pixel (#39)
- [x] 2.5 Measure the render floor through a re-mesh, so it bounds the noise it is compared against (#38)

## 3. What this change does not do

- [ ] 3.1 Wire `NoticeBoard`, `HistoryViewModel`, `JobRunner`, `UnitsModel` and `DiagnosticsModel` — five complete, tested ViewModels nothing constructs, which is why engine failures reach stderr instead of the sculptor
- [ ] 3.2 Work off the twenty-six domain labels the shell still draws untranslated, held from growing by a ratchet (#40)
- [ ] 3.3 Split `App` (58 fields, 78 methods) and `ClayDocument` (40 fields, nine traits). Both are real and neither is a line-count argument — `document.rs` is 6,792 lines and its functions never appear in a complexity ranking

## 4. Local gate reproducibility (#190)

- [x] 4.1 Check cached SDK, CMake and source paths before reusing a CyberRemesher CMake cache; give the cleaning command on a stale path, with regression tests.
- [x] 4.2 Verify `cargo clippy --workspace -- -D warnings` on a clean macOS checkout.
- [x] 4.3 Account for the four Linux failures in the archived `upgrade-engine-0-113-0` change and the later pin in archived `upgrade-engine-0-116-0` and `upgrade-engine-0-120-0` changes.
- [x] 4.4 Run the long-session visual fixture and keep its reproduced rendering defect as an enabled macOS tripwire until a clean-frame fix replaces it.
