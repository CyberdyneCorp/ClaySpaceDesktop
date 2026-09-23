## MODIFIED Requirements

### Requirement: A hierarchy is sculpted with the mesh vocabulary less its colour
The application SHALL offer, on a layer holding a subdivision hierarchy, the
fixed-topology brushes it offers on a mesh layer, together with the mask brush.
It SHALL NOT offer the brushes that write vertex colour. It SHALL additionally
offer the eraser, which is not a fixed-topology brush and reaches no mesh
layer: a hierarchy stores detail in channels that can be taken back one at a
time, and a mesh has one surface with nothing stored beneath it.

Which tools reach a hierarchy SHALL be derived from the same declared table
every other representation is derived from, and the count SHALL be asserted
against the engine's own vocabulary so that a verb the engine gains is a
failing count rather than a silence. Both differences from the mesh column —
the two colour brushes that are absent and the one eraser that is present —
SHALL be asserted by name, so that a third difference appearing is a failure
rather than a shelf nobody looked at.

#### Scenario: The mesh brushes reach a hierarchy
- **WHEN** the tools offered on a hierarchy are listed
- **THEN** they are the tools offered on a mesh layer, less the two colour
  brushes and plus the eraser

#### Scenario: A tool the mesh sculptor does not have is not invented here
- **WHEN** a tool is offered on a hierarchy and not on a mesh layer
- **THEN** it is the eraser, which the representation earns rather than
  inherits, and no other

#### Scenario: The mask is the same call wherever it is painted
- **WHEN** the mask brush is used on any representation
- **THEN** it invokes the same engine verb, because a mask belongs to none of
  them

## ADDED Requirements

### Requirement: Erasing on a hierarchy takes the selected pass toward zero
The application SHALL, when the eraser is used on a subdivision hierarchy, take
the selected pass's own detail toward zero, leaving the form beneath the passes
and every other pass exactly as they were.

It SHALL state on the tool, for a hierarchy alone, that this is what erasing
means there — the one label in the table over two different operations, since
on a grid the same tool clears the cells the brush covers and a hierarchy has
no cells to clear.

The erase SHALL be reported like any other stroke, so that a sculptor sees that
something happened even where the change is subtle, and one erase gesture SHALL
be one step in the edit history however many segments the drag arrived in.

#### Scenario: The pass goes and nothing else moves
- **WHEN** a sculptor erases over a region of a hierarchy with a pass selected
- **THEN** that pass's deposit is lowered in the region the brush covered
- **AND** the form beneath the passes and every other pass are unchanged

#### Scenario: One drag is one undo
- **WHEN** an erase gesture is made and then undone
- **THEN** the pass is back exactly as it was before the gesture, in one step

#### Scenario: The caveat is shown for the eraser on a hierarchy
- **WHEN** the eraser is shown against a hierarchy
- **THEN** its caveat says that erasing takes the selected pass toward zero

#### Scenario: The same tool on a grid carries no such caveat
- **WHEN** the eraser is shown against a voxel grid
- **THEN** no caveat is shown

### Requirement: Erasing is refused where the form is selected rather than a pass
The application SHALL refuse the eraser on a hierarchy whose selected row is
the form beneath the passes, and the refusal SHALL name the pass a sculptor has
to select. It SHALL NOT redirect the gesture to the form: walking the form's own
detail toward zero takes the whole surface back toward the pure subdivision,
which is a different operation at a scale an eraser does not suggest.

The refusal SHALL be a state of the layer rather than an absence from the
shelf, so the eraser stays visible and carries its reason, as every other tool
that exists for the active representation and cannot be used right now does.

The rule SHALL reach the eraser on a hierarchy and no other pair, since no
other representation has a row to select and no other verb means something
different in one row than in the other.

#### Scenario: The form is not a pass
- **WHEN** a sculptor selects the form's row on a hierarchy and erases
- **THEN** the stroke is refused, the refusal names a pass, and the surface
  does not move

#### Scenario: Selecting a pass is all it takes
- **WHEN** the sculptor then selects a pass and erases again
- **THEN** the stroke is made

#### Scenario: No other tool asks about a pass
- **WHEN** any other tool is used on any representation with no pass selected
- **THEN** it is not refused for want of one
