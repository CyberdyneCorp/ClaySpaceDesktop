## ADDED Requirements

### Requirement: Layer names are unambiguous and bounded
The application SHALL require layer names to be unique across every representation and no longer than 128 Unicode scalar values. Automatically generated names SHALL obey the same limit.

#### Scenario: Duplicate or oversized rename
- **WHEN** a user renames a layer to another layer's name or to a name longer than 128 characters
- **THEN** the rename is refused and the layer retains its original name

### Requirement: Selection refers to a current layer
An object selection SHALL refer only to a layer still present in the scene.

#### Scenario: In-place crossing removes a selected layer
- **WHEN** an in-place conversion removes the layer of a selected object
- **THEN** the selection is cleared

### Requirement: Hidden and locked refusals name their cause
An edit refusal SHALL distinguish a hidden layer from a locked layer.

#### Scenario: Hidden layer refusal
- **WHEN** an edit is attempted on a hidden layer
- **THEN** the refusal states that the layer is hidden

#### Scenario: Locked layer refusal
- **WHEN** an edit is attempted on a locked layer
- **THEN** the refusal states that the layer is locked
