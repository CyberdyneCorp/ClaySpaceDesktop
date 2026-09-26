## ADDED Requirements

### Requirement: A refusal names the reason and the representation that hold
A refusal SHALL name the reason that actually holds. A hidden layer SHALL be
refused as hidden and a ghosted or locked one as protected; visibility SHALL NOT
be folded into protection, so that neither reason masks the other.

A layer action that belongs to other representations SHALL be refused by naming
where it applies and what the layer is, as every tool refusal does. Optimizing
(the whole-layer bake) SHALL be refused on anything but a field layer, and
placing an object SHALL be refused on anything but a field layer, each naming
the layer's own representation.

#### Scenario: A hidden layer is refused as hidden
- **WHEN** a brush is applied to the active layer while it is hidden
- **THEN** no edit occurs and the refusal says the layer is hidden, not locked

#### Scenario: Optimizing a grid says it is a field's action
- **WHEN** the user optimizes a grid layer
- **THEN** nothing changes, no history entry is made, and the refusal says the
  action applies to field layers and that this one is a grid

#### Scenario: An object refused on a grid names the grid
- **WHEN** the user places an object while a grid layer is active
- **THEN** nothing is placed and the refusal names the grid as the active
  representation
