# Proposal

## Why

The scene can report objects, names, and selections that no longer match its document. Visibility changes and refusals can also leave the surface or explanation inconsistent with the layer state.

## What Changes

- Reconcile placed objects and selection after optimization and in-place conversion.
- Validate object selection and require unique, bounded layer names.
- Restore the field after a hide/show cycle and distinguish hidden from locked refusals.

## Capabilities

### Modified Capabilities

- `scene-and-layers`: Layer naming, visibility, selection, and refusal invariants.
- `scene-objects`: Every listed object must refer to a live document node.

## Impact

The document adapter, scene and object view models, and their regression tests change. No persisted format changes.
