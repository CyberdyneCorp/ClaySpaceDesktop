## MODIFIED Requirements

### Requirement: Documents are saved and opened in the engine's format
The application SHALL open and save `.clayspace` documents through the engine's
document I/O. It SHALL NOT define a container format of its own, and SHALL NOT
write engine documents by any path other than the engine's writer. A saved
document SHALL preserve its layers, their representation, stack order,
visibility, protection, names and transforms.

A mask painted on a subtool SHALL be part of the saved document and SHALL come
back covering the same region when the document is reopened. A document written
before masks were saved SHALL open with no mask rather than failing.

#### Scenario: Round-trip preserves the document
- **WHEN** a document with layers, masks, an armature and a mesh layer is saved and reopened
- **THEN** every layer, mask, armature node and mesh layer is present with its content and settings intact, in the same stack order and with the same representations, names, visibility and protection

#### Scenario: Documents are portable across platforms
- **WHEN** the same document is saved on macOS and on Linux from equivalent state
- **THEN** the two files are byte-identical

#### Scenario: A document from a newer engine is refused clearly
- **WHEN** the user opens a document whose scene chunk version is newer than the engine supports
- **THEN** the application refuses to open it and states the version mismatch, rather than opening it partially

#### Scenario: A mask survives the round trip
- **WHEN** a mask is painted on a subtool, the document is saved, closed and
  opened again
- **THEN** the mask covers the same region and still gates the brushes on that
  subtool

#### Scenario: A document with no mask opens unchanged
- **WHEN** a document written before masks were saved is opened
- **THEN** it opens normally and carries no mask
