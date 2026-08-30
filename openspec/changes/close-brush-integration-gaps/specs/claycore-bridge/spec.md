## MODIFIED Requirements

### Requirement: Handle ownership is expressed in the type system
Owned handles SHALL be released exactly once by RAII wrappers. Borrowed handles
SHALL NOT outlive their owner, enforced by lifetimes rather than by convention.

Where the engine's own entry point takes a document together with a handle that
document lends — a mask attached to one of its layers — the safe wrapper SHALL
address that handle by the **identity of the layer it belongs to** rather than
lending it to the caller and asking for it back. A borrowed mask SHALL NOT have
to escape into a caller's mutation path for that caller to use it.

#### Scenario: A borrowed mask cannot outlive its document
- **WHEN** code attempts to hold a document-owned mask past the document
- **THEN** it does not compile

#### Scenario: A layer's own mask gates an edit to that layer
- **WHEN** a mask is attached to a layer and a stroke is applied to the same
  layer naming that layer as the mask source
- **THEN** the frozen region is unchanged, and the caller never handled the mask

## ADDED Requirements

### Requirement: The topological move is reachable
The wrapper SHALL bind the engine's topological move, which drags a volume with
a falloff measured along the material rather than through space, with the
anchor, reach, displacement and easing the engine's descriptor takes.

#### Scenario: Reach is measured along the surface
- **WHEN** a topological move is applied to a volume whose two parts are close
  in space and far along the surface, with a radius smaller than the path
  between them
- **THEN** only the part containing the anchor moves

#### Scenario: A volume is required
- **WHEN** the move is applied to an item that carries no volume
- **THEN** the call returns an error rather than silently doing nothing
