# Proposal

## Why

Curve controls can report success without changing the visible tube. A non-circle radius edit only changes guide radii while the placed profile keeps its previous parameters; the curve manipulator target has no transform; and inactive curve commands silently succeed.

## What Changes

- Rebuild non-circle profile items when their radius changes, preserving a single undoable edit.
- Expose the selected control points as a manipulator target. Move, rotate, and scale resolve from the gesture's starting points and draw at their centroid.
- Refuse curve edits when no curve is active, and refuse a curve gizmo target without selected points.
- Guard profile changes so the selected join takes effect immediately; keep profile and control state in sync through undo and redo.

## Capabilities

### Modified Capabilities

- `object-transform`: Curve control points can be transformed by the visible manipulator.
- `sculpting-tools`: Curve controls update the tube immediately and inactive edits are refused.

## Impact

Curve document editing and history, the object ViewModel, desktop gizmo targeting, regression tests, and feature documentation.
