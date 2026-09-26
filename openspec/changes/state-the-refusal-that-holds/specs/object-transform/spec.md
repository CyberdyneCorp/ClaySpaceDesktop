## ADDED Requirements

### Requirement: A manipulation that would move nothing is refused
A cage or manipulator request that cannot move anything SHALL be refused with a
stated reason rather than accepted as though it had moved something:

- a direct cage drag with no cage up, or with other than exactly one control
  point selected;
- a cage rotation or scale with fewer than two control points selected, since
  one point is its own pivot;
- a manipulator target the document does not hold — an object or a layer that
  is not in it, or a curve with no control point selected.

A refusal SHALL leave the cage and the target as they stood.

#### Scenario: A direct drag needs exactly one point
- **WHEN** a cage drag is asked for with none or several control points selected
- **THEN** it is refused, and no control point moves

#### Scenario: One point is not turned or scaled
- **WHEN** a cage rotation or scale is dragged with one control point selected
- **THEN** it is refused, and the point stays where it was; moving it still works

#### Scenario: A target that is not in the document is refused
- **WHEN** the manipulator is pointed at an object that is not in the document
- **THEN** no target is held, the refusal says so, and a following drag begins
  nothing
