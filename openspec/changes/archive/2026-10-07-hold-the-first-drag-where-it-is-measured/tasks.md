## 1. Profile the first drag (D14)

- [x] 1.1 Split the first mesh subtool drag frame and the first placed-object press by phase, against the second and third, in release and in debug; figures in the proposal and in the two tests' doc comments.
- [x] 1.2 Time the first `transform/drag` after `transform/set_mode` through the door on an SDF layer, a mesh crossed from the field and a placed object; figures in the proposal.

## 2. Hold the budget where it is measured

- [x] 2.1 `support::Verdict`, `verdict_for`, `budget_verdict` and `hold_to_budget` in `crates/clayspace-app/tests/support/mod.rs`.
- [x] 2.2 `first_mesh_subtool_drag_frame_fits_the_frame_budget` and `the_first_object_drag_frame_draws_the_object_alone` hold their budgets through it; the release budget and the structural assertions are unchanged.
- [x] 2.3 `a_millisecond_budget_is_a_verdict_only_in_release_off_a_hosted_runner` in `gizmo_first_drag.rs`.
- [x] 2.4 `visual_incremental.rs`: `exact()` allows 128 pixels in a debug build or on a hosted runner (`CI`), 16 on a workstation in release, at the same levels, with the runners' measured figures in its doc comment.

## 3. Close I14

- [x] 3.1 Record the two non-reproductions on main and the tripwire `a_grid_display_change_with_no_grid_in_view_does_no_work` as the reason the row is closed, in the proposal and in `docs/features.md`.

## 4. Documentation

- [x] 4.1 `docs/features.md`: the profile of the first drag and where its budget is held; I14 closed.
- [x] 4.2 `docs/roadmap.md`: a budget asserted where it measures the runner, under *Seven ways a gate can be real and unenforced*.
- [x] 4.3 `performance-budgets`: a millisecond budget is a verdict in an optimised build off a hosted runner and a figure elsewhere.

## 5. Verification

- [x] 5.1 `cargo test -p clayspace-app --release --test gizmo_first_drag --test visual_incremental`, the same with `CI=true`, and the same in debug.
- [x] 5.2 `cargo test --workspace --release` once; the gates in the justfile.
