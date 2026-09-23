## MODIFIED Requirements

### Requirement: The mask operations take an amount the interface can set
The application SHALL let a sculptor set how far Expandir, Contrair and
Suavizar máscara reach, and what an extrusion's thickness, rim rounding and rim
smoothing are, and SHALL apply those amounts rather than fixed defaults.

Each menu entry SHALL show the amount it would apply.

The amount applied SHALL be the one the **command carries**. The menu fills it
in from the panel before it dispatches, which is where a menu entry gets to
spell out what it would do, and a caller at the agent door names its own. A
ViewModel that wrote the panel's amount over whatever arrived made `steps` a
parameter the door accepted and ignored.

An amount outside what the engine accepts SHALL be brought inside it, by the
same bounds the panel's own control uses, and SHALL be reported rather than
applied silently.

#### Scenario: An expansion reaches as far as the panel says
- **WHEN** the amount is set to four and Expandir is chosen
- **THEN** the frozen region grows further than it would at one

#### Scenario: An expansion reaches as far as a caller asked
- **WHEN** an expansion of four steps is asked for through the agent door while
  the panel stands at one
- **THEN** the region grows by four

#### Scenario: An amount the engine would refuse is brought in and reported
- **WHEN** an operation is asked for with no steps or with more than the engine
  takes
- **THEN** the operation is applied at the nearest amount that means something
  and the caller is told the amount was changed

#### Scenario: An extrusion is as thick as the panel says
- **WHEN** the thickness is set and the patch is extruded outward
- **THEN** the wall stands that far off the surface

#### Scenario: An operation with no amount is left alone
- **WHEN** an amount is set and Inverter is chosen
- **THEN** the operation carries no amount

## ADDED Requirements

### Requirement: Clearing a mask that freezes nothing costs nothing
Limpar is offered whether or not anything is frozen, because pressing it should
do the obvious nothing rather than be greyed out with a reason nobody needs.
That nothing SHALL cost nothing: where the active subtool freezes nothing, the
application SHALL NOT write to the mask, SHALL NOT send the frozen region to be
drawn again, and SHALL NOT record anything to take back.

It is not a refusal. The mask ends up exactly as the caller asked for it, so the
application SHALL say there was nothing to clear as a remark.

#### Scenario: Clearing an empty mask writes nothing
- **WHEN** Limpar is chosen on a subtool that freezes nothing
- **THEN** nothing is uploaded, nothing is added to the history, and the
  caller is told there was nothing to clear
