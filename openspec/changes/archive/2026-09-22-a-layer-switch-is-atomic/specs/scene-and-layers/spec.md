## ADDED Requirements

### Requirement: Changing the active layer takes effect at once
Moving the sculpt target SHALL be atomic: when the command returns, every part
of the interface that describes the active subtool SHALL describe the new one.
The shelf, the active tool, the brush settings, the symmetry toggles and the
mask state SHALL all have followed before the next command is handled, so that
the **first** stroke after a switch is made with the new subtool's settings.

Commands that move the sculpt target SHALL include choosing a layer, adding one
and removing one. A new layer arrives active, and a removal hands the target to
whatever layer is left, which may hold a different representation.

A newly added layer SHALL NOT inherit brush settings from another
representation. Settings are held per tool and per representation, and the size
shown for the new layer SHALL be the size its next stroke uses.

A subtool that carries no mask SHALL report none as soon as it becomes active,
rather than continuing to report the mask of the subtool left behind. Moving
the sculpt target is not an edit, so this SHALL NOT depend on any later
document change to be observed.

A new armature layer SHALL start with its own symmetry rather than the previous
subtool's. The rig places its own reflected nodes, so the layer it is given is
created with its mirror off, and the interface SHALL show and use that mirror
from the moment the layer exists.

#### Scenario: The first stroke after a switch uses the new layer's brush
- **WHEN** a brush size is set on one subtool, another subtool is selected, and
  a stroke is made
- **THEN** the stroke is made with the newly selected subtool's brush size,
  tool and symmetry, not the previous subtool's

#### Scenario: A new layer does not inherit a brush size
- **WHEN** a grid layer is added while a field layer with its own brush size is
  active
- **THEN** the brush size reported for the new grid layer is the grid's own,
  and it is the size the next dab is made at

#### Scenario: Mask state follows the active subtool
- **WHEN** a mask is painted on one subtool and another subtool with no mask
  becomes active
- **THEN** the mask state reports no mask immediately, without any further
  command

#### Scenario: A new rig layer is not mirrored
- **WHEN** a rig is started while the active subtool has symmetry on
- **THEN** the new armature layer reports symmetry off, and the first ZSphere
  is placed unmirrored
