# Proposal

## Why

ClayCore v0.120.1 records voxel sculpt-layer creation and operations in its undo history. The desktop's separate pass history now duplicates those entries and can leave the surface and displayed strength disagreeing after undo.

## What Changes

- Use the engine's undo entries for voxel pass operations and creation.
- Refresh each grid's displayed pass stack after an engine undo or redo, including when a different layer is selected.
- Make removal and merge-down undoable, as the new engine release supports.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `voxel-sculpt-layers`: Pass creation, removal, and merge-down are undoable; undo and redo keep the pass controls and surface in sync.

## Impact

`crates/clayspace-engine` history and pass cache, application undo behavior, voxel pass tests, and the ClayCore v0.120.1 submodule pin.
