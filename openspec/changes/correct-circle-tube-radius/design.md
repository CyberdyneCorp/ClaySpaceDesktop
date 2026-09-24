# Design

ClayCore's `ctape_stroke_dist` evaluates every round-cone segment and chooses between a smooth minimum and a hard minimum based on `stroke_blend_k`. The smooth minimum accumulates negative distance where adjacent segments overlap. The engine currently sets `k` to half the smallest curve point radius, which makes the measured width depend on tessellation density.

Set `stroke_blend_k` to zero on the circle curve item. The hard minimum is the union of the segments, so a straight chain of equal-radius capsules has a constant radius independent of point count. This setting is local to curves; Snake Hook and other stroke users keep their blend.

Measure the evaluated field with binary search along a transverse ray, away from the starting form. This tests the geometry directly instead of relying on rendered pixels or guide point values. Sweep radius 0.02, 0.1 and 0.5 with 2, 10 and 50 control points. Check straight-span positions under every join. For square, hexagon and triangle profiles, compare the same radial directions at multiple positions along a dense straight fixture to detect longitudinal spikes without treating intended polygon corners as defects.
