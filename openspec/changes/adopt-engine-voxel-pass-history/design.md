# Design

## Context

The desktop kept a host-side `PassEdit` stack because earlier ClayCore releases did not record voxel pass operations. ClayCore v0.120.1 records pass creation, changes, removal, merge-down, and edits made inside a pass. The duplicate host entry causes the undo command to choose the host inverse rather than the engine snapshot. The engine restores the grid, but the desktop also caches pass metadata for the scene panel.

## Decision

Remove the host-side pass history. Keep the carried mesh-gesture history, which ClayCore still does not own. After each engine undo or redo, refresh pass metadata for every voxel layer. Reading every stack is necessary because the undone entry may belong to a layer that is no longer selected. Grid geometry remains on its existing dirty-chunk and smooth-mesh paths.

## Validation

Check a pass dial through the app's command banking and direct engine history tests. Verify creation is one step; exercise changes interleaved with other edits. Run the workspace suite and CI matrix against both new submodule pins.
