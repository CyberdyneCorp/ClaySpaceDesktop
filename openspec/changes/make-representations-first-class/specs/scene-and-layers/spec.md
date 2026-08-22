## MODIFIED Requirements

### Requirement: Mesh layers are carried and sculpted, but do not compose
The application SHALL allow an imported mesh to be carried by the document as a
mesh layer, saved and reloaded with it, and exported alongside sculpted content.
A mesh layer SHALL be sculptable in place with the engine's fixed-topology
brushes — see the `mesh-sculpting` capability — and SHALL be pickable in the
viewport.

A mesh layer SHALL NOT be usable as an operand of a boolean, a blend or a
deformer belonging to another layer. Where a user asks for one, the application
SHALL state that composing requires conversion and SHALL name what that
conversion costs.

This replaces the previous rule that sculpting tools are disabled on mesh
layers. That rule described the engine as it was: a mesh layer's triangles were
read-only, and the only way to edit one discarded the edge loops and UVs that
made it worth importing. Sixteen fixed-topology brushes now reach a mesh layer's
own vertices without touching its topology, so the refusal describes nothing
that is still true. What has not changed is composability, and that is the line
the requirement now draws.

#### Scenario: A mesh layer round-trips
- **WHEN** a document containing an imported mesh layer is saved and reopened
- **THEN** the mesh layer is present with its geometry unchanged

#### Scenario: A mesh layer is sculpted
- **WHEN** a mesh layer is active and the user selects a mesh brush
- **THEN** the brush is available and a stroke moves the mesh's vertices

#### Scenario: Composing with a mesh layer is refused with a route
- **WHEN** the user asks to subtract a mesh layer from another layer
- **THEN** the application states that a mesh layer is not an operand, offers
  the conversion that would make it one, and names what the conversion costs
