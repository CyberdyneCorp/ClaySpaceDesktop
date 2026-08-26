## ADDED Requirements

### Requirement: A mesh offered as a boolean operand is converted on use
Where a sculptor uses a mesh layer as the operand of a boolean, the application
SHALL offer to convert it rather than refusing, SHALL state the same costs the
conversion panel states before it runs, and SHALL leave the source mesh layer
where it is.

A mesh cannot compose: it is not an operand of a boolean belonging to another
layer until it is crossed to a field, and paying that crossing quantises the
vertices and drops the edge loops that made it worth keeping as a mesh. That
remains true. What changes is that the sculptor meets it as an offer with a
price on it, at the moment they are trying to use the mesh, rather than as a
refusal pointing at a panel elsewhere.

#### Scenario: A custom object is subtracted
- **WHEN** the user chooses an imported mesh as the shape to subtract
- **THEN** the crossing's costs are stated, and on accepting them a converted
  copy becomes the operand while the mesh layer stays as it was

#### Scenario: The costs are the panel's own
- **WHEN** the costs are stated for a conversion on use
- **THEN** they are the same figures the conversion panel computes for that
  crossing at that resolution

#### Scenario: Declining leaves everything alone
- **WHEN** the user declines the conversion
- **THEN** no layer is added, the mesh is unchanged, and no boolean is made
