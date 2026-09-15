## ADDED Requirements

### Requirement: Compact exact triangle duplicate keys
The surface store SHALL identify duplicate triangles using exact equality of all vertex attributes and SHALL preserve the original pruning result when using temporary vertex IDs.

#### Scenario: Equal triangle under multiple keys
- **WHEN** identical full-attribute triangles occur under several brick keys or index permutations
- **THEN** the first occurrence in sorted brick traversal survives
- **AND** surviving indices and all vertex arrays retain their original order and bits

#### Scenario: Attributes differ
- **WHEN** triangles share positions but differ in normal, color, mask, signed-zero bits or NaN payloads
- **THEN** differing vertex keys remain distinct exactly as in the original pruning algorithm

### Requirement: Pruning uses bounded temporary storage
Pruning SHALL allocate its vertex and triangle lookup tables only for the current stored surface and SHALL release them when compaction finishes.

#### Scenario: Empty store
- **WHEN** no triangles are stored
- **THEN** pruning leaves the store unchanged

### Requirement: Hashing preserves exact identity
Temporary lookup tables SHALL resolve collisions by full key equality and SHALL produce the same geometry independently of randomized hash seeds.

#### Scenario: Distinct vertices collide
- **WHEN** distinct complete vertex keys and triangle ID triples receive identical hashes
- **THEN** distinct triangles survive and exact duplicates are removed
- **AND** complete vertex bits, surviving indices and ownership match the original pruning algorithm
