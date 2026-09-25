# Proposal

## Why

Dragging or appending a curve point leaves stale geometry in the brick cache. Local point and span bounds do not cover every changed location because ClayCore's curve field can depend on the full guide.

## What Changes

- Mark the old and new curve extents for each guide edit before draining the cache.
- Skip a zero-displacement drag.
- Compare cached geometry with a full refill across joins and radii, and render a drag fixture.
- Document the correctness and performance tradeoff.

## Capabilities

### Modified Capabilities

- `sculpting-tools`: Curve edits leave the cache consistent with a full refill.

## Impact

Curve refill logic and regressions in `clayspace-engine`, rendered regression in `clayspace-app`, and curve documentation.
