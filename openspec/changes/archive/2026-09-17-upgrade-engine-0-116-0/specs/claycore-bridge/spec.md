## MODIFIED Requirements

### Requirement: The topological move is reachable
The wrapper SHALL bind the engine's topological move, which drags a volume with
a falloff measured along the material rather than through space, with the
anchor, reach, displacement and easing the engine's descriptor takes.

The moved volume SHALL be written back with the same feather every other
replacing edit uses. A hard `CLAY_OP_REPLACE` edge is a step in the field, and a
step in the field is a lattice on the surface — the inside is the volume, the
outside is whatever was there, and nothing crosses between them. With a feather
the band between the two crosses over, which is the whole of ClayCore #67.

#### Scenario: Reach is measured along the surface
- **WHEN** a topological move is applied to a volume whose two parts are close
  in space and far along the surface, with a radius smaller than the path
  between them
- **THEN** only the part containing the anchor moves

#### Scenario: A volume is required
- **WHEN** the move is applied to an item that carries no volume
- **THEN** the call returns an error rather than silently doing nothing

#### Scenario: A drag the move made leaves no lattice
- **WHEN** a drag is made on a field through the topological move and the
  surface is looked at
- **THEN** it carries no grid of steps at the edge of what the move replaced

## ADDED Requirements

### Requirement: A second vendored engine is pinned and checked the same way
The retopology and UV library SHALL be vendored as a git submodule at
`vendor/CyberRemesherAndUV`, pinned to a release tag, and advancing it SHALL be
a reviewed change rather than an automatic update — as the ClayCore pin is.

Two separate assertions SHALL hold it, because they answer two questions and a
library can satisfy one and still be wrong:

- The library's **declared ABI** SHALL be checked through the library's own
  compatibility call rather than by comparing two numbers in this workspace.
  The question is whether the library can serve a client compiled against
  *these* headers, which is not the question "are these two numbers equal".
- The library's **release number** SHALL be asserted against the pin, because
  the ABI check says the library can serve this build and says nothing about
  the submodule being the build we meant to pin.

Neither number SHALL be read back from the library it checks.

#### Scenario: The pin moves and the release assertion does not
- **WHEN** the submodule is pointed at a release whose version differs from the
  constant
- **THEN** the guard fails, naming both

#### Scenario: A library that cannot serve these headers
- **WHEN** the linked library declares an ABI its own compatibility rule
  refuses for the headers this build compiled against
- **THEN** the check fails rather than the workspace linking and finding out
  later
