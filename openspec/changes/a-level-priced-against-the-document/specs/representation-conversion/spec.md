## MODIFIED Requirements

### Requirement: Subdividing is priced on what the build holds at its worst
The application SHALL state, before a level is added, how many vertices and
faces it would create, what it would hold afterwards, and the high-water mark
during the build. It SHALL refuse a level on the high-water mark rather than on
what remains, and SHALL refuse one past the depth the engine takes.

The high-water mark SHALL be priced on top of what the document already holds —
every layer, every surface beside it and every level the hierarchy already has
— rather than against an empty machine, and a refusal SHALL name what is held,
what the level adds and the budget, so that the sum can be checked.

The faces quoted SHALL be the faces of the cage the hierarchy was built over,
multiplied as the subdivision rule multiplies them: a triangle's first step
makes three faces and every step after it four. A cage taken from a mesh layer
is the layer's triangulation, so a retopology of quads is priced from twice as
many triangles.

#### Scenario: A level that fits once built and not while building is refused
- **WHEN** a level whose peak allocation exceeds the budget but whose persistent
  cost does not is priced
- **THEN** it is refused, and the refusal names the peak and the budget

#### Scenario: A level that fits an empty document and not this one is refused
- **WHEN** a level whose peak fits the budget on its own is asked for on a
  document already holding enough that the two together do not
- **THEN** it is refused, the refusal names what the document holds, the peak
  and the budget, and the hierarchy holds the levels it held before

#### Scenario: The depth ceiling is a refusal rather than a failure
- **WHEN** a hierarchy already as deep as the engine takes is asked for another
  level
- **THEN** it is refused, and the refusal names how deep it already is

#### Scenario: A face count that would overflow does not report as affordable
- **WHEN** the faces a great many further subdivisions would produce are
  projected
- **THEN** the projection saturates rather than wrapping to a small number

#### Scenario: The faces quoted follow from the cage's own faces
- **WHEN** a level is priced over a cage of triangles
- **THEN** the first level quoted is three times the cage's faces and each level
  after it four times the one below, and the level built holds exactly the
  faces quoted
