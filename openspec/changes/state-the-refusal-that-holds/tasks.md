# Tasks

## 1. Refusals that name what holds

- [x] 1.1 Engine answers `active_layer_visible`; `active_layer_editable` is
      protection alone (`tests/solo.rs`)
- [x] 1.2 `consolidate_layer` refuses non-field layers with `NoVerbHere`
      (`tests/optimize_routing.rs`)
- [x] 1.3 Object refusal names the active representation
      (`clayspace-vm/tests/objects.rs`)

## 2. Manipulations that move nothing

- [x] 2.1 `drag_lattice_point` refuses no cage and a selection other than one
      point (`tests/lattice.rs`)
- [x] 2.2 Cage rotate/scale refuses a single point (`tests/lattice.rs`)
- [x] 2.3 `SetGizmoTarget` refuses a target the model cannot place
      (`clayspace-vm/tests/objects.rs`)

## 3. Readouts

- [x] 3.1 Uniform scale reads as one number in the transform readout
- [x] 3.2 Leaving a hierarchy names its levels in the crossing cost
