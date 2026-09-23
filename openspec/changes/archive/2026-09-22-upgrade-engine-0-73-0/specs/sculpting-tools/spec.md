## MODIFIED Requirements

### Requirement: Masks gate operations, not only brushes
The application SHALL apply a painted mask to any operation the engine can gate,
including combine operations, and not only to brush strokes.

The gate SHALL be set on the stroke's own template, which is correct for every
stamp the stroke deposits because the engine measures a gate in **world space**:
the region it protects is where the mask was painted and stays there whatever
placement the gated item is then given. This is the opposite of the alpha rule,
where a deformer is resolved in the item's own frame and so cannot be carried by
a template — the two must not be reasoned about together.

A gate the engine refuses SHALL leave the stamp ungated rather than failing the
stroke. The engine refuses a gate that would protect nothing — an empty mask, or
one no cell of which reaches the threshold — and an ungated stamp is the correct
outcome in exactly that case.

Protection SHALL fade across a stated width rather than at a step. A gate is a
measured distance and not the painted mask, so painted softness is re-derived
from that width; a hard edge has no finite Lipschitz bound and nothing could
march it.

#### Scenario: A mask protects against a boolean
- **WHEN** a region is masked and a subtracting edit crosses it
- **THEN** the masked region is not cut

#### Scenario: An unmasked document is unaffected
- **WHEN** a subtracting stroke is made on a layer carrying no mask
- **THEN** the stroke is not refused and cuts as it always did

#### Scenario: Masking still keeps a brush from depositing
- **WHEN** a depositing stroke crosses a masked region wider than the brush
- **THEN** the masked region is not deposited into
