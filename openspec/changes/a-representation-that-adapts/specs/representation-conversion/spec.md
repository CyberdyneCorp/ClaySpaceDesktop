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
