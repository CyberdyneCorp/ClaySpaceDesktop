## ADDED Requirements

### Requirement: A Dynamic stroke is benchmarked at two sizes
The benchmark SHALL measure a Dynamic stroke on a fixture at two sizes of the
same density, 100,352 and 1,002,528 triangles: the steady dab of an open
stroke with the surface arriving, the drawing half on its own, and the bytes
each dab uploads. The drawing half SHALL be held to the 16 ms frame budget at
both sizes, the whole dab SHALL be held to it on the smaller fixture, and the
larger fixture's bytes per dab SHALL be at most twice the smaller's.

#### Scenario: Upload volume does not follow the model
- **WHEN** the `dynamic` group runs
- **THEN** `dynamic.upload_scaling` is at most 2 and `dynamic.upload_100k` and `dynamic.upload_1m` are within 16 ms
