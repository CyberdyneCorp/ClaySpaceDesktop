# Correct circle tube radius

## Why

Circle tubes use a smoothly blended chain of overlapping round-cone segments. Every overlap adds volume, so a dense guide makes a tube wider than its authored radius. On the pinned ClayCore release, a straight 50-point guide at radius 0.02 measured a 0.0385 half-width.

## What changes

- Combine segments within a circle sweep with a hard union, preserving each guide point's radius.
- Measure field zero crossings across supported radii, guide densities, joins and positions along a straight span.
- Guard non-circle profiles against longitudinal corner spikes on a dense straight fixture.

## Impact

Previously saved circle tubes evaluate thinner after this change, at the radius their controls already stated. The curve's existing undo, guide and profile behavior remains in place.
