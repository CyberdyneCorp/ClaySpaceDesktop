## ADDED Requirements

### Requirement: Optimising a brush-chain layer refuses while a regional bake would slow it
The layer-level optimise action SHALL refuse a field layer degraded by a chain of brushes for as long as a regional bake of that layer measures dearer to refill than the chain it would replace, and SHALL say that flattening it would make strokes slower rather than faster.

The regional bake was expected to replace this refusal. It was measured instead: on the pinned engine a baked patch costs about sixty times the analytic chain per brick refilled at the brick cache's spacing, and an undo on the collapsed layer went from 61 ms to 3.6 s. The refusal is therefore the correct answer on this pin, and is kept until the tripwire that measures the difference fails.

#### Scenario: A layer degraded by a chain of brushes
- **WHEN** a sculptor optimises a field layer whose degradation is a deformer chain
- **THEN** the action refuses, the document is unchanged, and the sentence says that flattening it would make strokes slower

#### Scenario: A layer degraded by something other than a chain
- **WHEN** the layer's degradation is a stack of volumes or a long edit list
- **THEN** the whole-layer collapse is still what runs, unchanged

### Requirement: The whole-layer bake states what it costs in detail
Where the application offers to optimise a field layer, the offer SHALL say that the bake resamples the subtool at the surface's own spacing and that detail finer than one sample is softened, in every locale the interface speaks, so that a sculptor accepts a stated tolerance rather than discovering one.

#### Scenario: A layer the bake would help
- **WHEN** a field layer's degradation is a stack of volumes or a long edit list, and the interface offers to optimise it
- **THEN** the offer's hint says the subtool is resampled at the surface's own spacing and that detail finer than one sample is softened

#### Scenario: Every locale says it
- **WHEN** the interface is in any of its locales
- **THEN** the hint is present and translated rather than copied from another locale
