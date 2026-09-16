## MODIFIED Requirements

### Requirement: Optimising a brush-chain layer compacts it rather than refusing
The layer-level optimise action SHALL collapse the region of the most recent gesture on a field layer degraded by a chain of brushes, and SHALL refuse only where no region is available to collapse.

Previously it refused outright on such a layer, because the whole-layer bake it invoked is measured 6x worse for a deformer chain and it had no gesture to take a region from.

#### Scenario: A layer degraded by a chain of brushes
- **WHEN** a sculptor optimises a field layer whose degradation is a deformer chain, and a gesture has been made on it
- **THEN** the region of that gesture is collapsed
- **AND** the action does not refuse

#### Scenario: No gesture to take a region from
- **WHEN** the same layer has had no gesture in this session, so no region is known
- **THEN** the action refuses and says that it has no worked region to compact, rather than collapsing the whole layer

#### Scenario: A layer degraded by something other than a chain
- **WHEN** the layer's degradation is a stack of volumes or a long edit list
- **THEN** the whole-layer collapse is still what runs, unchanged
