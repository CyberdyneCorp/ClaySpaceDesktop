## MODIFIED Requirements

### Requirement: A voxel grid can be repaired before baking
The application SHALL offer the engine's pre-bake repair on a voxel layer:
reporting what is wrong, closing holes, and filling voids. The report SHALL be
shown before any repair is applied.

Closing holes SHALL close single-cell perforations and every opening up to
twice its reach across that leads into a hollow, where the reach is three cells
at one pass and one cell more for each further pass. It SHALL NOT fill a dent,
a groove, the hole of a ring or a wider opening, and SHALL NOT fill a hollow
that is already sealed, which is what filling voids is for.

Each repair SHALL state, once it has run, how many holes or voids it found, how
many it closed, how many remain, and how many cells it added, in the repair
panel and to an agent. A repair that adds no cell SHALL report no change and
record no history entry, and closing holes SHALL be one undo step.

#### Scenario: A report precedes a repair
- **WHEN** the user opens repair on a voxel layer
- **THEN** the count of holes and voids is stated before anything is changed

#### Scenario: Holes are closed
- **WHEN** the user closes holes on a pierced shell
- **THEN** the report afterwards states fewer holes

#### Scenario: A hole wider than a cell is closed and counted
- **WHEN** the user closes holes on a shell pierced by a hole three cells across
- **THEN** the hole's cells are filled, the repair states one hole found, one
  closed and none remaining, and one undo takes the repair back

#### Scenario: A wide opening is left alone
- **WHEN** the user closes holes on a shell whose whole top is open
- **THEN** no cell is added and the repair states no hole found

#### Scenario: Filling voids is counted
- **WHEN** the user fills voids in a sealed hollow shell
- **THEN** the repair states one void found, one closed and none remaining
