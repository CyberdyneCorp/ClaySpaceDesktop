## ADDED Requirements

### Requirement: Dynamic conversion is explicit and preflighted
Mesh → Dynamic and Dynamic → Mesh SHALL run only on an explicit request.
Dynamic → Mesh SHALL use the engine's preflight and report conversion cost,
limitations and refusal before changing the document. A successful crossing
SHALL form one undo entry and leave selection pointing to a live layer. A
refusal SHALL leave representation, selection and history unchanged.

#### Scenario: A brush does not convert the layer
- **WHEN** a requested brush has no binding on a mesh layer but has one on Dynamic
- **THEN** the application reports the missing binding without converting the mesh

#### Scenario: A refused bake changes nothing
- **WHEN** Dynamic → Mesh preflight refuses the current surface
- **THEN** no conversion occurs and the reason is shown

#### Scenario: A successful conversion is undoable
- **WHEN** a mesh is explicitly converted to Dynamic and then undone
- **THEN** the original mesh and its selection are restored in one history step

#### Scenario: Undoing a freeze restores the adaptive surface
- **WHEN** a sculpted Dynamic layer is frozen to a mesh and the freeze is undone
- **THEN** the layer is Dynamic again with the sculpted connectivity, not an empty or fixed layer

### Requirement: Dynamic conversion is priced against a budget
Both Dynamic crossings SHALL be priced by the engine's preflight before
anything is built, and the conversion panel SHALL show the priced peak beside
the budget. A crossing whose peak, added to what the document already holds,
exceeds the budget SHALL be refused naming the direction, the peak, the held
figure and the budget, and SHALL change nothing.

#### Scenario: An over-budget crossing is refused with its figures
- **WHEN** either Dynamic crossing is requested with a budget below its priced peak
- **THEN** it is refused with the engine's peak and the budget, and the rows, selection, geometry and history are unchanged

#### Scenario: The panel states the price before the crossing
- **WHEN** the conversion panel is set to a Dynamic crossing
- **THEN** it shows the engine's peak and the budget, and the crossing into Dynamic says quads become triangles

### Requirement: Dynamic conversion keeps the layer's identity
A Dynamic crossing SHALL carry the source's transform and visibility to the
result. A crossing that adds a layer SHALL keep the source beside it, and is
the default. A crossing back to a mesh in place SHALL restore the name the
mesh had before it was converted. A Mesh → Dynamic → Mesh round trip with no
edit between SHALL preserve every vertex within 1e-5 and the triangle count.

#### Scenario: The round trip keeps the form
- **WHEN** a mesh is converted to Dynamic and frozen back in place with no stroke between
- **THEN** its vertices match within 1e-5 and its triangle count is unchanged

#### Scenario: Transform, name and visibility survive
- **WHEN** a placed, hidden mesh is converted to Dynamic and frozen back in place
- **THEN** the result stands where the mesh stood, is still hidden, and has the mesh's name

#### Scenario: No tool converts a layer
- **WHEN** every tool is stroked on a layer of every representation
- **THEN** every layer keeps its representation and no row is added
