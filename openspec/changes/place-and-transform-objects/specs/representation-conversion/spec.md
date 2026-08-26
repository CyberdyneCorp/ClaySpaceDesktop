## MODIFIED Requirements

### Requirement: A conversion states its cost before it runs
The application SHALL state, before a conversion is performed, what that
conversion loses. The statement SHALL name the surface movement in terms of the
chosen cell size, the loss of features thinner than a cell, the loss of sharp
edges to a staircase, and the loss of the procedural edit history where the
direction discards it.

Every crossing that samples onto a lattice SHALL state those losses, and the
crossing from a mesh into a field is one of them. It samples the model onto a
lattice like the others; what differs is only that the resolution was chosen
for the sculptor rather than by them, which is a reason to offer the choice and
not a reason to price the crossing at nothing.

This replaces a rule that was true of what the application *displayed* and not
of what it did. Mesh-to-SDF was excluded from the directions that choose a
resolution, so a sculptor crossing an imported model into clay was told the
surface would not move, nothing would be lost, and its sharp edges would
survive — none of which is true of a lattice sampling. Found by placing a mesh
as a boolean operand, which pays the same crossing and was made to state the
same costs, and stated none.

#### Scenario: The cost is shown before committing
- **WHEN** the user opens the conversion for an SDF layer
- **THEN** the losses for that direction are stated, and the conversion has not
  yet run

#### Scenario: The cost reflects the chosen resolution
- **WHEN** the user changes the cell size in the conversion
- **THEN** the stated surface movement changes with it

#### Scenario: A mesh crossing into a field is priced like the others
- **WHEN** the losses are stated for a crossing from a mesh into a field
- **THEN** they name the surface movement, the vanishing feature size and the
  loss of sharp edges, on the same terms as a crossing into a grid

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
