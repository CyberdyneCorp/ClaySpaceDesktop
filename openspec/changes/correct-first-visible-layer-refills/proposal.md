# Proposal

## Why

Hiding the first visible field layer can promote a subtracting layer above it to the field initializer. That layer can then create surface outside the hidden layer's bound, while the desktop cache marks only the hidden layer. Reordering and removal can cause the same stale surface.

## What Changes

- Track the first visible SDF layer across visibility, removal, and reorder commands.
- Mark a composed layer whose initializer role changes, using the engine's own layer influence region.
- Preserve the current bounded refill when that role does not change and the zero field cost of grid and carried mesh visibility.
- Cover the cache against a full region rebuild and document the rule.

## Capabilities

### Modified Capabilities

- `scene-and-layers`: Visibility and stack edits keep the field cache accurate when a composed layer becomes or stops being the initializer.

## Impact

`crates/claycore` exposes the layer composition predicate through the C ABI; `crates/clayspace-engine` marks the additional region; cache regressions and layer documentation change.
