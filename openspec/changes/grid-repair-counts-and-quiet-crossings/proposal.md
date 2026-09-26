# Grid repair counts and quiet crossings

## Why

Issue #185 measured three grid problems. Close Holes filled single-cell
pinholes only, so a shell with a hole two or more cells across kept an opening
a sculptor could see through, and neither repair said what it had found or
closed — the pre-bake report counts enclosed voids, and a perforated shell has
none. And every crossing ended with a whole-surface settle of the brick field,
which re-meshed an unchanged source layer even when the crossing never touched
the field.

## What changes

- Close Holes also closes openings up to twice its reach across that lead into
  a hollow, decided by flooding rather than a local rule, and leaves dents,
  grooves, rings and wide openings alone.
- Both repairs state how many holes or voids they found, closed and left, and
  how many cells they added — in the repair panel and as a remark to the agent
  door. A repair that finds nothing is a no-op, not a change.
- Close Holes is one undo step.
- A crossing settles the brick surface only when it changes the field; any
  other crossing syncs incrementally.

Hidden grids are already skipped by a display change (#239), and that rule is
in `voxel-sculpt-layers`. The grid-to-field converter's own cost and grid mask
extrude are left for follow-up work.

## Capabilities

### Modified Capabilities

- `sculpting-tools`: the voxel repair requirement states the counts and
  through-hole closing.
- `representation-conversion`: a crossing that leaves the field unchanged does
  not re-mesh it.

## Impact

`clayspace-engine` gains a `holes` module and `ClayDocument::last_repair`;
`clayspace-model` gains `RepairOutcome` and `Direction::changes_the_field`; the
repair panel, the application's remark channels and its crossing path change.
No persisted format changes.
