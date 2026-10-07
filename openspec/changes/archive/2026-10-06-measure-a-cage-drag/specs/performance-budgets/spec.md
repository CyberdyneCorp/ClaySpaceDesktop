## ADDED Requirements

### Requirement: A cage drag has recorded benchmarks
The benchmark SHALL time a single-point drag frame on a mesh cage at the
smallest useful, the default and the largest cage size over the mesh reference,
from the pointer move to the surface arriving, and SHALL attach the 16 ms
interface-thread budget to each figure. It SHALL report the largest cage's
frame over the smallest's as a ratio with a budget of a small multiple, and the
device memory held after a 100-frame drag, released, over what was held before
it, with a budget of 1.2.

A long drag SHALL NOT grow the document's accounted memory with the number of
frames, and taking the cage down SHALL return it to within 20% of what it was
before the drag.

#### Scenario: The three cage sizes are measured
- **WHEN** the benchmark runs with a headless GPU
- **THEN** it reports `cage.drag_3.ms`, `cage.drag_8.ms` and `cage.drag_32.ms`, each against a 16 ms budget
- **AND** it reports `cage.scaling` and `cage.memory` against their budgets

#### Scenario: A long drag leaves nothing behind
- **WHEN** one control point of a mesh cage is dragged over a hundred frames and the cage is cancelled
- **THEN** the document's memory a hundred frames in is within 20% of its memory ten frames in
- **AND** after the cancel it is within 20% of its memory before the drag
