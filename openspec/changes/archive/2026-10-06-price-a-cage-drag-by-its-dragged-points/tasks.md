## 1. Engine

- [x] 1.1 ClayCore#655: sum a mesh cage over its dragged points with an O(n) basis per axis, with unit tests and `BM_MeshLatticeDrag`.
- [x] 1.2 Move the engine pin to the ClayCore release carrying 1.1 (v0.126.0, `upgrade-engine-0-126-0`).
- [x] 1.3 Flip `a_mesh_cage_evaluation_is_priced_by_every_point_the_cage_holds` to assert a 32³ and a 3³ cage cost within a small multiple of each other (now `a_mesh_cage_evaluation_is_priced_by_its_dragged_points`, line at 20x, measured 9x; done with the pin move).
- [x] 1.4 Assert the 16 ms frame for a single-point 32³ drag on the reference mesh (`one_corner_of_the_largest_cage_holds_a_frame`, 9.9 ms on the 62,576-vertex starting mesh at the CI gate's 10x margin; the `cage.drag_32` budget stays attached and the group's figures are re-measured in the proposal).
- [x] 1.5 Add the `cage` benchmark group at 3³, 8³ and 32³ with the frame budget attached (`measure-a-cage-drag`).

## 2. Reporting

- [x] 2.1 `LatticeState::dragged` and `LatticeState::preview_micros`, filled by the document from the cage and the timed preview frame.
- [x] 2.2 The agent's cage state carries `dragged_points` and `preview_ms`.

## 3. Verification

- [x] 3.1 `lattice.rs`: `preview_and_apply_agree`, `a_mesh_drag_reports_what_its_frame_cost_and_what_it_was_given`, `a_field_cage_reports_no_preview_cost`.
- [x] 3.2 `lattice.rs`: `a_cage_is_sized_from_where_the_form_stands_now`, `a_mirrored_form_is_caged_with_its_mirror`.
- [x] 3.3 `report.rs`: `a_previewed_cage_reports_what_its_last_frame_cost`.
