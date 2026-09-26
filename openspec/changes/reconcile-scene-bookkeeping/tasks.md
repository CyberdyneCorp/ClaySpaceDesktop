# Tasks

- [x] Reconcile objects and selection after optimization, with undo coverage.
- [x] Refuse unknown object selections.
- [x] Enforce unique names of at most 128 Unicode scalar values across layer representations.
- [x] Restore the drawn surface after hiding and showing a field subtool.
- [x] Clear stale selection after an in-place crossing.
- [x] Distinguish hidden and locked refusals and correct representation messages.
- [x] Add focused regression tests and validate OpenSpec and Rust tests.

The current tool availability and scene view models already distinguish voxel from SDF and hidden from locked. Regression coverage for those paths was confirmed and tightened for the insert refusal.
